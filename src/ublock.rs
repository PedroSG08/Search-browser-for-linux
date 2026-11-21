// uBlock Origin's filter lists, compiled to WebKit content blockers.
//
// WebKitGTK has no chrome.* / browser.* extension API, so uBlock Origin
// itself cannot be installed as an extension here. What WebKitGTK does have
// is the same content-blocker engine Safari uses: rules are compiled once,
// then enforced inside the network and style layers before a request is made
// or a stylesheet is applied. This module turns the official uBO lists into
// that format.
//
// It is deliberately a translator, not another hand-written blocklist. The
// sources below are the lists uBlock Origin ships with. Rules the WebKit
// format cannot express (response-body replacement, redirects, URL
// stripping, procedural cosmetic filters) are left out rather than guessed
// at, so a page is never broken by a half-understood rule.

use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::rc::Weak;

use glib::translate::*;
use webkit2gtk::ffi;

use crate::browser::Browser;
use crate::store;

pub const FILTER_ID: &str = "search-ublock-v1";

/// Bump this when the bundled lists change. The compiled WebKit filter is
/// reused while the marker matches, so ordinary launches do not parse the
/// lists at all.
const LISTS_VERSION: &str = "2026-11-18.3";

const LISTS: &[(&str, &str)] = &[
    (
        "uBlock filters",
        include_str!("../assets/ublock/ublock-filters.txt"),
    ),
    (
        "uBlock filters – Badware risks",
        include_str!("../assets/ublock/ublock-badware.txt"),
    ),
    (
        "uBlock filters – Privacy",
        include_str!("../assets/ublock/ublock-privacy.txt"),
    ),
    (
        "uBlock filters – Quick fixes",
        include_str!("../assets/ublock/ublock-quick-fixes.txt"),
    ),
    (
        "uBlock filters – Unbreak",
        include_str!("../assets/ublock/ublock-unbreak.txt"),
    ),
    ("EasyList", include_str!("../assets/ublock/easylist.txt")),
    ("EasyPrivacy", include_str!("../assets/ublock/easyprivacy.txt")),
    (
        "Peter Lowe's Ad and tracking server list",
        include_str!("../assets/ublock/peter-lowe.txt"),
    ),
];

// --------------------------------------------------------------- translation

fn json_string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into())
}

fn push_array(out: &mut String, key: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    out.push_str(",\"");
    out.push_str(key);
    out.push_str("\":[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&json_string(value));
    }
    out.push(']');
}

#[derive(Default)]
struct Options {
    load: Option<&'static str>,
    types: Vec<&'static str>,
    if_domains: Vec<String>,
    unless_domains: Vec<String>,
    document: bool,
    match_case: bool,
}

impl Options {
    fn add_type(&mut self, kind: &'static str) {
        if !self.types.contains(&kind) {
            self.types.push(kind);
        }
    }
}

/// Split an ABP rule into its pattern and its `$options` tail. A `$` in the
/// middle of a URL is rare, and a suffix that looks like options always wins;
/// an unknown option makes the rule skipped instead of misconverted.
fn split_options(line: &str) -> (&str, Option<&str>) {
    let mut end = line.len();
    while let Some(position) = line[..end].rfind('$') {
        let suffix = &line[position + 1..];
        if !suffix.is_empty() {
            let first = suffix.split(',').next().unwrap_or("").trim();
            let name = first
                .split('=')
                .next()
                .unwrap_or("")
                .trim()
                .trim_start_matches('~')
                .to_lowercase();
            if known_option(&name) {
                return (&line[..position], Some(suffix));
            }
        }
        end = position;
    }
    (line, None)
}

fn normalize_domain(domain: &str) -> Option<String> {
    let mut text = domain.trim().to_lowercase();
    if text.is_empty() {
        return None;
    }
    if let Some(rest) = text.strip_prefix("~") {
        text = rest.to_string();
    }
    while text.starts_with('*') || text.starts_with('.') {
        if let Some(rest) = text.strip_prefix("*.") {
            text = rest.to_string();
        } else {
            text.remove(0);
        }
    }
    if let Some(stripped) = text.strip_suffix('^') {
        text = stripped.to_string();
    }
    if text.is_empty()
        || text.contains('/')
        || text.contains('*')
        || text.contains(' ')
        || text.contains('\\')
        || text.contains('"')
        || text.contains('[')
        || text.contains(']')
        || text.contains(':')
    {
        return None;
    }
    Some(text)
}

/// WebKit's `if-domain` / `unless-domain`: `*example.com` also matches every
/// subdomain. An IP address or `localhost` is left exact.
fn webkit_domain(domain: &str) -> String {
    let domain = domain.trim().to_lowercase();
    if domain.starts_with('*') {
        return domain;
    }
    let numeric = domain
        .chars()
        .all(|character| character.is_ascii_digit() || character == '.' || character == ':');
    if numeric || domain == "localhost" {
        domain
    } else {
        format!("*{}", domain)
    }
}

fn clean_domains(mut ifs: Vec<String>, mut unless: Vec<String>) -> Option<(Vec<String>, Vec<String>)> {
    ifs.sort();
    ifs.dedup();
    unless.sort();
    unless.dedup();
    unless.retain(|domain| !ifs.contains(domain));
    // WebKit refuses a trigger that has both if-domain and unless-domain.
    // Dropping the rule is safer than silently overriding an exception.
    if !ifs.is_empty() && !unless.is_empty() {
        return None;
    }
    ifs = ifs.iter().map(|domain| webkit_domain(domain)).collect();
    unless = unless.iter().map(|domain| webkit_domain(domain)).collect();
    Some((ifs, unless))
}

fn split_domains(value: &str) -> Option<(Vec<String>, Vec<String>)> {
    let mut ifs = Vec::new();
    let mut unless = Vec::new();
    for part in value.split('|') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(negative) = part.strip_prefix('~') {
            unless.push(normalize_domain(negative)?);
        } else {
            ifs.push(normalize_domain(part)?);
        }
    }
    if ifs.is_empty() && unless.is_empty() {
        return None;
    }
    Some((ifs, unless))
}

fn known_option(name: &str) -> bool {
    matches!(
        name,
        "third-party"
            | "3p"
            | "first-party"
            | "1p"
            | "strict3p"
            | "strict1p"
            | "script"
            | "image"
            | "stylesheet"
            | "css"
            | "document"
            | "doc"
            | "subdocument"
            | "frame"
            | "xhr"
            | "xmlhttprequest"
            | "media"
            | "font"
            | "object"
            | "object-subrequest"
            | "other"
            | "ping"
            | "websocket"
            | "beacon"
            | "popup"
            | "popunder"
            | "generichide"
            | "ghide"
            | "elemhide"
            | "ehide"
            | "specifichide"
            | "shide"
            | "important"
            | "match-case"
            | "domain"
            | "from"
            | "to"
            | "redirect"
            | "redirect-rule"
            | "replace"
            | "csp"
            | "removeparam"
            | "removeheader"
            | "header"
            | "rewrite"
            | "urltransform"
            | "uritransform"
            | "cname"
            | "badfilter"
            | "all"
            | "method"
            | "denyallow"
            | "ipaddress"
            | "urlskip"
            | "reason"
            | "permissions"
            | "webrtc"
            | "inline-script"
            | "inline-font"
    )
}

/// Parse the `$options` of a filter. `None` means the rule uses something the
/// WebKit format cannot express and must be skipped.
fn parse_options(suffix: &str) -> Option<Options> {
    let mut options = Options::default();
    for token in suffix.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let mut pieces = token.splitn(2, '=');
        let name = pieces.next().unwrap_or("").trim();
        let value = pieces.next().unwrap_or("");
        let negative = name.starts_with('~');
        let name = name.trim_start_matches('~').to_lowercase();
        if !known_option(&name) {
            return None;
        }
        match name.as_str() {
            "third-party" | "3p" | "strict3p" => {
                options.load = Some(if negative { "first-party" } else { "third-party" });
            }
            "first-party" | "1p" | "strict1p" => {
                options.load = Some(if negative { "third-party" } else { "first-party" });
            }
            "domain" | "from" => {
                let (ifs, unless) = split_domains(value)?;
                options.if_domains.extend(ifs);
                options.unless_domains.extend(unless);
            }
            "script" => {
                if negative {
                    return None;
                }
                options.add_type("script");
            }
            "image" => {
                if negative {
                    return None;
                }
                options.add_type("image");
            }
            "stylesheet" | "css" => {
                if negative {
                    return None;
                }
                options.add_type("style-sheet");
            }
            "document" | "doc" => {
                if negative {
                    return None;
                }
                options.document = true;
            }
            "subdocument" | "frame" => {
                if negative {
                    return None;
                }
                options.add_type("document");
            }
            "xhr" | "xmlhttprequest" => {
                if negative {
                    return None;
                }
                options.add_type("raw");
            }
            "media" => {
                if negative {
                    return None;
                }
                options.add_type("media");
            }
            "font" => {
                if negative {
                    return None;
                }
                options.add_type("font");
            }
            "object" | "object-subrequest" | "other" | "ping" | "websocket" | "beacon" => {
                if negative {
                    return None;
                }
                options.add_type("raw");
            }
            "popup" | "popunder" => {
                if negative {
                    return None;
                }
                options.add_type("popup");
            }
            "all" => {
                if negative {
                    return None;
                }
            }
            "important" => {}
            "match-case" => options.match_case = true,
            // Request rewriting, redirects, CSP injection, URL stripping and
            // procedural cosmetics have no content-blocker equivalent.
            "redirect"
            | "redirect-rule"
            | "replace"
            | "csp"
            | "removeparam"
            | "removeheader"
            | "header"
            | "rewrite"
            | "urltransform"
            | "uritransform"
            | "cname"
            | "badfilter"
            | "method"
            | "denyallow"
            | "ipaddress"
            | "urlskip"
            | "reason"
            | "permissions"
            | "webrtc"
            | "inline-script"
            | "inline-font"
            | "generichide"
            | "ghide"
            | "elemhide"
            | "ehide"
            | "specifichide"
            | "shide"
            | "to" => return None,
            _ => return None,
        }
    }
    Some(options)
}

/// Translate an ABP URL pattern into the regular expression WebKit stores in
/// a content-blocker trigger.
fn translate_pattern(pattern: &str) -> Option<String> {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return None;
    }
    if pattern.len() >= 3 && pattern.starts_with('/') && pattern.ends_with('/') {
        // WebKit's content-blocker regex engine is a restricted subset. It
        // rejects disjunctions and lookarounds, which appear in many list
        // regexes; there are only a few dozen of them, so they are skipped.
        return None;
    }

    let mut rest = pattern;
    let mut out = String::new();
    let mut anchored_start = false;
    let anchored_end = rest.ends_with('|');
    if rest.starts_with("||") {
        out.push_str("^[a-z][a-z0-9+.-]*://([^/]+\\.)?");
        anchored_start = true;
        rest = &rest[2..];
    } else if rest.starts_with('|') {
        out.push('^');
        anchored_start = true;
        rest = &rest[1..];
    }
    if anchored_end {
        rest = &rest[..rest.len() - 1];
    }
    if rest.is_empty() {
        return None;
    }
    for character in rest.chars() {
        match character {
            '*' => out.push_str(".*"),
            '^' => out.push_str("[^A-Za-z0-9_\\-.%]"),
            '.' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' => {
                out.push('\\');
                out.push(character);
            }
            '\\' => out.push_str("\\\\"),
            '$' => out.push_str("\\$"),
            '|' => out.push_str("\\|"),
            _ => out.push(character),
        }
    }
    if anchored_end {
        out.push('$');
    }
    if !anchored_start {
        out.insert_str(0, ".*");
        if !anchored_end {
            out.push_str(".*");
        }
    }
    if out.len() > 1800 || out.contains('\n') || out.contains('\r') {
        return None;
    }
    Some(out)
}

fn block_rule(url: &str, options: &Options, action: &str) -> Option<String> {
    let (if_domains, unless_domains) = clean_domains(
        options.if_domains.clone(),
        options.unless_domains.clone(),
    )?;
    let mut trigger = String::from("{\"url-filter\":");
    trigger.push_str(&json_string(url));
    if options.match_case {
        trigger.push_str(",\"url-filter-is-case-sensitive\":true");
    }
    if let Some(load) = options.load {
        trigger.push_str(",\"load-type\":[");
        trigger.push_str(&json_string(load));
        trigger.push(']');
    }
    if !options.types.is_empty() {
        let types: Vec<String> = options
            .types
            .iter()
            .map(|kind| (*kind).to_string())
            .collect();
        push_array(&mut trigger, "resource-type", &types);
    }
    push_array(&mut trigger, "if-domain", &if_domains);
    push_array(&mut trigger, "unless-domain", &unless_domains);
    trigger.push('}');
    Some(format!(
        "{{\"trigger\":{},\"action\":{{\"type\":{}}}}}",
        trigger,
        json_string(action)
    ))
}

fn cosmetic_rule(
    selector: &str,
    if_domains: &[String],
    unless_domains: &[String],
) -> Option<String> {
    if selector.trim().is_empty() {
        return None;
    }
    let mut trigger = String::from("{\"url-filter\":\".*\"");
    push_array(&mut trigger, "if-domain", if_domains);
    push_array(&mut trigger, "unless-domain", unless_domains);
    trigger.push('}');
    let mut rule = String::from("{\"trigger\":");
    rule.push_str(&trigger);
    rule.push_str(",\"action\":{\"type\":\"css-display-none\",\"selector\":");
    rule.push_str(&json_string(selector));
    rule.push_str("}}");
    Some(rule)
}

fn host_from_pattern(pattern: &str) -> Option<String> {
    let rest = pattern
        .trim()
        .trim_start_matches("||")
        .trim_start_matches('|');
    let host = rest.split(['^', '/', '$', '*', '?']).next()?.trim();
    if host.contains('.') && !host.contains(' ') {
        normalize_domain(host)
    } else {
        None
    }
}

// ------------------------------------------------------------------- builder

#[derive(Default)]
struct Builder {
    rules: Vec<String>,
    exceptions: Vec<String>,
    doc_whitelist: Vec<String>,
    doc_domains: HashSet<String>,
    genhide: HashSet<String>,
    elemhide: HashSet<String>,
    cosmetic_sites: Vec<(Vec<String>, String)>,
    cosmetic_exceptions: Vec<(Vec<String>, String)>,
    generic: HashSet<String>,
    network_count: usize,
    cosmetic_count: usize,
    exception_count: usize,
}

impl Builder {
    fn parse_list(&mut self, text: &str) {
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('!') || line.starts_with('[') {
                continue;
            }
            if line.starts_with("@@") {
                if self.cosmetic_exception(&line[2..]) {
                    continue;
                }
                self.network(&line[2..], true);
                continue;
            }
            if let Some((domains, marker, selector)) = split_cosmetic(line) {
                match marker {
                    "##" => self.cosmetic(domains, selector),
                    "#@#" => self.cosmetic_exception_selector(domains, selector),
                    _ => {}
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("0.0.0.0 ") {
                self.hosts(rest);
                continue;
            }
            if let Some(rest) = line.strip_prefix("127.0.0.1 ") {
                self.hosts(rest);
                continue;
            }
            self.network(line, false);
        }
    }

    fn hosts(&mut self, value: &str) {
        let Some(host) = value.split_whitespace().next() else {
            return;
        };
        let host = host.trim().to_lowercase();
        if host.is_empty() || host == "localhost" || host == "0.0.0.0" {
            return;
        }
        let escaped = host
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || character == '-' || character == '.' {
                    character.to_string()
                } else {
                    format!("\\{}", character)
                }
            })
            .collect::<String>();
        let url = format!("^https?://([^/]+\\.)?{}([:/])", escaped);
        let options = Options::default();
        if let Some(rule) = block_rule(&url, &options, "block") {
            self.rules.push(rule);
            self.network_count += 1;
        }
    }

    /// Collect the site lists of cosmetic exceptions and tell the caller
    /// whether the line was purely a cosmetic exception (so it should not
    /// also be turned into a network rule).
    fn cosmetic_exception(&mut self, source: &str) -> bool {
        let (pattern, suffix) = split_options(source);
        let Some(suffix) = suffix else {
            return false;
        };
        let mut generic = false;
        let mut element = false;
        let mut other = false;
        for token in suffix.split(',') {
            let name = token
                .split('=')
                .next()
                .unwrap_or("")
                .trim()
                .trim_start_matches('~')
                .to_lowercase();
            match name.as_str() {
                // `elemhide` disables both generic and site-specific hiding.
                "elemhide" | "ehide" | "specifichide" | "shide" => {
                    generic = true;
                    element = true;
                }
                "generichide" | "ghide" => generic = true,
                "domain" | "from" => {}
                _ => other = true,
            }
        }
        if (!generic && !element) || other {
            return false;
        }
        let mut domains: Vec<String> = Vec::new();
        for token in suffix.split(',') {
            let token = token.trim();
            if let Some(value) = token
                .strip_prefix("domain=")
                .or_else(|| token.strip_prefix("from="))
            {
                let Some((ifs, _)) = split_domains(value) else {
                    return false;
                };
                domains.extend(ifs);
            }
        }
        if domains.is_empty() {
            if let Some(host) = host_from_pattern(pattern) {
                domains.push(host);
            }
        }
        for domain in domains {
            if generic {
                self.genhide.insert(domain.clone());
            }
            if element {
                self.elemhide.insert(domain);
            }
        }
        true
    }

    fn network(&mut self, source: &str, exception: bool) {
        let (pattern, suffix) = split_options(source);
        let mut options = Options::default();
        if let Some(suffix) = suffix {
            let Some(parsed) = parse_options(suffix) else {
                return;
            };
            options = parsed;
        }

        // A document exception switches off every rule on that site. WebKit
        // expresses that as a final ignore-previous-rules with a top-site
        // domain condition.
        if exception && options.document {
            let mut domains = options.if_domains.clone();
            if domains.is_empty() {
                if let Some(host) = host_from_pattern(pattern) {
                    domains.push(host);
                }
            }
            for domain in domains {
                if self.doc_domains.insert(domain.clone()) {
                    let mut trigger = String::from("{\"url-filter\":\".*\"");
                    let domain = webkit_domain(&domain);
                    push_array(&mut trigger, "if-domain", &[domain]);
                    trigger.push('}');
                    self.doc_whitelist.push(format!(
                        "{{\"trigger\":{},\"action\":{{\"type\":\"ignore-previous-rules\"}}}}",
                        trigger
                    ));
                }
            }
            return;
        }

        if options.document {
            options.add_type("document");
        }
        let Some(url) = translate_pattern(pattern) else {
            return;
        };
        // A `*` pattern with no domain condition blocks everything on every
        // site — that is not expressible safely in WebKit's per-trigger model
        // (one condition only) and would break the web. Those rules are
        // uBlock-origin shorthand for "block on these sites" and already
        // appear with an `if-domain`; a naked `*` is skipped.
        if url == ".*.*.*" && options.if_domains.is_empty() && options.unless_domains.is_empty() {
            return;
        }
        let action = if exception {
            "ignore-previous-rules"
        } else {
            "block"
        };
        let Some(rule) = block_rule(&url, &options, action) else {
            return;
        };
        if exception {
            self.exceptions.push(rule);
            self.exception_count += 1;
        } else {
            self.rules.push(rule);
            self.network_count += 1;
        }
    }

    fn cosmetic(&mut self, raw_domains: &str, raw_selector: &str) {
        let selector = raw_selector.trim();
        if selector.is_empty() || selector.len() > 500 {
            return;
        }
        if selector.starts_with('^') {
            return;
        }
        if selector.contains('`')
            || selector.contains('{')
            || selector.contains('}')
            || selector.contains(":-abp-")
            || selector.contains(":-safari-")
            || selector.contains(":has(")
            || selector.contains(":contains(")
            || selector.contains(":matches(")
            || selector.contains(":matches-css")
            || selector.contains(":xpath(")
            || selector.contains(":style(")
            || selector.contains(":remove(")
            || selector.contains(":upward(")
            || selector.contains(":nth-ancestor(")
            || selector.contains(":watch-attr(")
            || selector.contains(":others(")
            || selector.contains(":if(")
            || selector.contains(":if-not(")
            || selector.contains(":min-text-length(")
            || selector.contains(":matches-attr(")
            || selector.contains(":matches-path(")
            || selector.contains(":matches-prop(")
            || selector.contains(":shadow(")
            || selector.contains(":remove-attr(")
            || selector.contains(":remove-class(")
            || selector.contains(":spath(")
            || selector.contains("+js(")
        {
            return;
        }
        if selector.matches('(').count() != selector.matches(')').count()
            || selector.matches('[').count() != selector.matches(']').count()
        {
            return;
        }
        let mut domains: Vec<String> = Vec::new();
        for part in raw_domains.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if part.starts_with('~') {
                // A negative-only cosmetic rule is applied everywhere except
                // one site; WebKit can express that, but mixing it with a
                // positive list cannot. Skip rather than over-hide.
                return;
            }
            if let Some(domain) = normalize_domain(part) {
                domains.push(domain);
            }
        }
        if domains.is_empty() {
            self.generic.insert(selector.to_string());
        } else if domains.len() <= 100 {
            self.cosmetic_sites.push((domains, selector.to_string()));
        }
    }

    fn cosmetic_exception_selector(&mut self, raw_domains: &str, raw_selector: &str) {
        let selector = raw_selector.trim();
        if selector.is_empty() {
            return;
        }
        let mut domains: Vec<String> = Vec::new();
        for part in raw_domains.split(',') {
            let part = part.trim();
            if part.is_empty() || part.starts_with('~') {
                continue;
            }
            if let Some(domain) = normalize_domain(part) {
                domains.push(domain);
            }
        }
        if !domains.is_empty() {
            self.cosmetic_exceptions
                .push((domains, selector.to_string()));
        }
    }

    fn finish(mut self) -> Vec<String> {
        // Selector-level exceptions (`site#@#.selector`) keep a rule from
        // hiding something on that site, but leave it in place elsewhere.
        let mut selector_exceptions: HashMap<String, HashSet<String>> = HashMap::new();
        for (domains, selector) in self.cosmetic_exceptions.drain(..) {
            selector_exceptions
                .entry(selector)
                .or_default()
                .extend(domains);
        }
        // Element hiding exceptions: drop the site-specific rules and add
        // the sites to the generic rule's unless-domain list.
        let elemhide = self.elemhide.clone();
        let mut cosmetic_sites = Vec::new();
        for (domains, selector) in self.cosmetic_sites.drain(..) {
            let excluded = selector_exceptions.get(&selector);
            let kept: Vec<String> = domains
                .into_iter()
                .filter(|domain| !elemhide.contains(domain))
                .filter(|domain| {
                    excluded
                        .map(|excluded| !excluded.contains(domain))
                        .unwrap_or(true)
                })
                .collect();
            if !kept.is_empty() {
                cosmetic_sites.push((kept, selector));
            }
        }
        // Stay safely below WebKit's rule ceiling. Network rules do the
        // heavy lifting; extra site cosmetics are the first thing to trim.
        let site_limit = 20_000usize;
        if cosmetic_sites.len() > site_limit {
            cosmetic_sites.truncate(site_limit);
        }
        for (domains, selector) in cosmetic_sites {
            let if_domains: Vec<String> = domains.iter().map(|domain| webkit_domain(domain)).collect();
            if let Some(rule) = cosmetic_rule(&selector, &if_domains, &[]) {
                self.rules.push(rule);
                self.cosmetic_count += 1;
            }
        }

        // Generic selectors with per-site exceptions get their own rule, so
        // the exception travels with the selector instead of disabling it
        // everywhere.
        let mut generic: Vec<String> = Vec::new();
        for selector in self.generic.into_iter() {
            match selector_exceptions.get(&selector) {
                Some(excluded) => {
                    let mut unless: Vec<String> = excluded
                        .iter()
                        .map(|domain| webkit_domain(domain))
                        .collect();
                    unless.sort();
                    unless.dedup();
                    if let Some(rule) = cosmetic_rule(&selector, &[], &unless) {
                        self.rules.push(rule);
                        self.cosmetic_count += 1;
                    }
                }
                None => generic.push(selector),
            }
        }
        generic.sort();
        let mut unless: Vec<String> = self
            .genhide
            .iter()
            .map(|domain| webkit_domain(domain))
            .collect();
        unless.sort();
        unless.dedup();
        for chunk in generic.chunks(250) {
            let selector = chunk.join(", ");
            if let Some(rule) = cosmetic_rule(&selector, &[], &unless) {
                self.rules.push(rule);
                self.cosmetic_count += 1;
            }
        }
        let _ = &self.exceptions;
        let _ = &self.doc_whitelist;
        let mut all = self.rules;
        all.extend(self.exceptions);
        all.extend(self.doc_whitelist);
        all
    }
}

fn split_cosmetic(line: &str) -> Option<(&str, &str, &str)> {
    let mut best: Option<(usize, &str)> = None;
    for marker in ["#@?#", "#$?#", "#@%#", "#@#", "#?#", "#$#", "#%#", "##"] {
        if let Some(position) = line.find(marker) {
            if best.map(|(at, _)| position < at).unwrap_or(true) {
                best = Some((position, marker));
            }
        }
    }
    let (position, marker) = best?;
    Some((
        &line[..position],
        marker,
        &line[position + marker.len()..],
    ))
}

pub fn rules_json() -> String {
    let mut builder = Builder::default();
    for (_, text) in LISTS {
        builder.parse_list(text);
    }
    let rules = builder.finish();
    let count = rules.len();
    let json = {
        let mut out = String::with_capacity(count * 180);
        out.push('[');
        for (index, rule) in rules.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str(rule);
        }
        out.push(']');
        out
    };
    eprintln!(
        "Search: uBlock lists compiled to {} rules ({} bytes)",
        count,
        json.len()
    );
    json
}

/// Diagnostic used by `search --ublock-stats`.
pub fn summary() -> String {
    let mut builder = Builder::default();
    for (_, text) in LISTS {
        builder.parse_list(text);
    }
    let network = builder.network_count;
    let exceptions = builder.exception_count;
    let cosmetics = builder.cosmetic_count + builder.cosmetic_sites.len();
    let generic = builder.generic.len();
    let rules = builder.finish();
    format!(
        "{} rules total · {} network · {} exceptions · {} cosmetic site rules · {} cosmetic selectors",
        rules.len(),
        network,
        exceptions,
        cosmetics,
        generic
    )
}

// ----------------------------------------------------------- WebKit filter

struct Payload {
    weak: Weak<Browser>,
    store: *mut ffi::WebKitUserContentFilterStore,
    json: Option<String>,
}

pub fn prepare(store: *mut ffi::WebKitUserContentFilterStore, weak: Weak<Browser>) {
    if store.is_null() {
        return;
    }
    let marker = store::data_dir().join("ublock.version");
    let current = store::read_file(&marker)
        .map(|text| text.trim().to_string())
        .unwrap_or_default();
    let payload = Box::into_raw(Box::new(Payload {
        weak,
        store,
        json: None,
    }));
    unsafe {
        if current == LISTS_VERSION {
            load(payload);
        } else {
            save(Box::from_raw(payload));
        }
    }
}

unsafe fn load(payload: *mut Payload) {
    let identifier = CString::new(FILTER_ID).unwrap();
    ffi::webkit_user_content_filter_store_load(
        (*payload).store,
        identifier.as_ptr(),
        std::ptr::null_mut(),
        Some(loaded),
        payload as glib::ffi::gpointer,
    );
}

unsafe fn save(mut payload: Box<Payload>) {
    if payload.json.is_none() {
        payload.json = Some(rules_json());
    }
    let identifier = CString::new(FILTER_ID).unwrap();
    let json = payload.json.as_deref().unwrap_or("[]");
    let bytes = glib::Bytes::from(json.as_bytes());
    ffi::webkit_user_content_filter_store_save(
        payload.store,
        identifier.as_ptr(),
        bytes.to_glib_none().0,
        std::ptr::null_mut(),
        Some(saved),
        Box::into_raw(payload) as glib::ffi::gpointer,
    );
}

unsafe fn deliver(payload: Box<Payload>, filter: *mut ffi::WebKitUserContentFilter) {
    if filter.is_null() {
        if let Some(browser) = payload.weak.upgrade() {
            browser.ublock_failed();
        }
        return;
    }
    match payload.weak.upgrade() {
        Some(browser) => {
            store::write_atomic(&store::data_dir().join("ublock.version"), LISTS_VERSION);
            eprintln!("Search: uBlock filter active");
            browser.ublock_ready(filter);
        }
        None => ffi::webkit_user_content_filter_unref(filter),
    }
}

unsafe extern "C" fn loaded(
    _source: *mut glib::gobject_ffi::GObject,
    result: *mut gio::ffi::GAsyncResult,
    user_data: glib::ffi::gpointer,
) {
    let payload = Box::from_raw(user_data as *mut Payload);
    let mut error: *mut glib::ffi::GError = std::ptr::null_mut();
    let filter =
        ffi::webkit_user_content_filter_store_load_finish(payload.store, result, &mut error);
    if !error.is_null() {
        glib::ffi::g_error_free(error);
        save(payload);
        return;
    }
    if filter.is_null() {
        save(payload);
        return;
    }
    deliver(payload, filter);
}

unsafe extern "C" fn saved(
    _source: *mut glib::gobject_ffi::GObject,
    result: *mut gio::ffi::GAsyncResult,
    user_data: glib::ffi::gpointer,
) {
    let payload = Box::from_raw(user_data as *mut Payload);
    let mut error: *mut glib::ffi::GError = std::ptr::null_mut();
    let filter =
        ffi::webkit_user_content_filter_store_save_finish(payload.store, result, &mut error);
    if !error.is_null() {
        let message = std::ffi::CStr::from_ptr((*error).message)
            .to_string_lossy()
            .into_owned();
        eprintln!("Search: the uBlock filter lists would not compile: {}", message);
        glib::ffi::g_error_free(error);
    }
    if !filter.is_null() {
        deliver(payload, filter);
    } else if let Some(browser) = payload.weak.upgrade() {
        browser.ublock_failed();
    }
}

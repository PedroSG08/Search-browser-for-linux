// Settings and the small files Search keeps: where you have been, what you
// kept, the tabs that were open, the parts of pages you hid, and — encrypted
// with a key only this user can read — your passwords.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine as _;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

// ---------------------------------------------------------------- filesystem

pub fn home_dir() -> PathBuf {
    glib::home_dir()
}

pub fn data_dir() -> PathBuf {
    glib::user_data_dir().join("Search")
}

pub fn config_dir() -> PathBuf {
    glib::user_config_dir().join("Search")
}

pub fn downloads_dir() -> PathBuf {
    glib::user_special_dir(glib::UserDirectory::Downloads)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| home_dir().join("Downloads"))
}

pub fn ensure_dir(path: &Path) {
    let _ = fs::create_dir_all(path);
}

pub fn read_file(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

pub fn write_private(path: &Path, contents: &[u8]) -> bool {
    if let Some(parent) = path.parent() {
        ensure_dir(parent);
    }
    let temp = path.with_extension(format!(
        "{}.tmp.{}",
        path.extension().and_then(|e| e.to_str()).unwrap_or(""),
        std::process::id()
    ));
    let ok = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&temp)?;
        file.write_all(contents)?;
        file.sync_all()?;
        fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))?;
        fs::rename(&temp, path)?;
        Ok(())
    })()
    .is_ok();
    if !ok {
        let _ = fs::remove_file(&temp);
    }
    ok
}

pub fn write_atomic(path: &Path, contents: &str) -> bool {
    if let Some(parent) = path.parent() {
        ensure_dir(parent);
    }
    let temp = path.with_extension(format!(
        "{}.tmp.{}",
        path.extension().and_then(|e| e.to_str()).unwrap_or(""),
        std::process::id()
    ));
    let ok = fs::write(&temp, contents)
        .and_then(|_| fs::rename(&temp, path))
        .is_ok();
    if !ok {
        let _ = fs::remove_file(&temp);
    }
    ok
}

/// Serialize straight into the temporary file instead of building the whole
/// JSON string in memory first. The bytes are the same as `to_string`, and
/// the rename still makes the replacement atomic.
fn write_json_atomic<T: Serialize + ?Sized>(path: &Path, value: &T) {
    if let Some(parent) = path.parent() {
        ensure_dir(parent);
    }
    let temp = path.with_extension(format!(
        "{}.tmp.{}",
        path.extension().and_then(|e| e.to_str()).unwrap_or(""),
        std::process::id()
    ));
    let result = (|| -> std::io::Result<()> {
        let file = fs::File::create(&temp)?;
        let mut writer = std::io::BufWriter::new(file);
        serde_json::to_writer(&mut writer, value).map_err(std::io::Error::other)?;
        writer.flush()?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
}

pub fn data_file(name: &str) -> PathBuf {
    data_dir().join(name)
}

// ------------------------------------------------------------------- strings

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn now_iso8601() -> String {
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::gmtime_r(&t, &mut tm);
        let mut buf = [0i8; 32];
        libc::strftime(
            buf.as_mut_ptr(),
            buf.len(),
            c"%Y-%m-%dT%H:%M:%SZ".as_ptr(),
            &tm,
        );
        std::ffi::CStr::from_ptr(buf.as_ptr())
            .to_string_lossy()
            .into_owned()
    }
}

pub fn uuid4() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

pub fn html_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn js_quote(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into())
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", value, UNITS[unit])
    }
}

pub fn time_ago(when: i64) -> String {
    let seconds = now_unix() - when;
    if seconds < 60 {
        "just now".into()
    } else if seconds < 3600 {
        format!("{} min ago", seconds / 60)
    } else if seconds < 86_400 {
        format!("{} hr ago", seconds / 3600)
    } else if seconds < 86_400 * 7 {
        format!("{} days ago", seconds / 86_400)
    } else if seconds < 86_400 * 30 {
        format!("{} weeks ago", seconds / (86_400 * 7))
    } else if seconds < 86_400 * 365 {
        format!("{} months ago", seconds / (86_400 * 30))
    } else {
        format!("{} years ago", seconds / (86_400 * 365))
    }
}

pub fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

// ---------------------------------------------------------------------- URLs

fn private_range(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    let mut octets = [0u8; 4];
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() || part.len() > 3 || !part.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        match part.parse::<u16>() {
            Ok(n) if n <= 255 => octets[i] = n as u8,
            _ => return false,
        }
    }
    octets[0] == 10
        || (octets[0] == 192 && octets[1] == 168)
        || (octets[0] == 172 && (16..=31).contains(&octets[1]))
}

fn looks_like_host(host: &str) -> bool {
    if host == "localhost" {
        return true;
    }
    let numbers: Vec<&str> = host.split('.').collect();
    if numbers.len() == 4
        && numbers
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 3 && p.parse::<u16>().map(|n| n <= 255).unwrap_or(false))
    {
        return true;
    }
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    if !labels.iter().all(|label| {
        !label.is_empty()
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    }) {
        return false;
    }
    let tld = labels.last().copied().unwrap_or("");
    tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic())
}

/// Whether what is typed is a place rather than words. Used to decide whether
/// a scheme should be added, or whether the words go to the search engine.
pub fn looks_like_url(typed: &str) -> bool {
    let text = typed.trim();
    if text.is_empty() || text.contains(' ') {
        return false;
    }
    let low = text.to_lowercase();
    if low.contains("://")
        || low.starts_with("about:")
        || low.starts_with("data:")
        || low.starts_with("file:")
        || low.starts_with("search:")
    {
        return true;
    }
    if text.starts_with('/') && !text.starts_with("//") {
        return true;
    }
    if text.starts_with("~/") {
        return true;
    }
    let head = text.split(['/', '?', '#']).next().unwrap_or(text);
    if head.contains('@') {
        return false;
    }
    if head.starts_with("[::1]") {
        return true;
    }
    let host = head.split(':').next().unwrap_or(head);
    looks_like_host(host)
}

/// A typed place with the scheme it needs. Paths and local names get http;
/// everything else https, as in Search on the Mac.
pub fn url_from_typed(typed: &str) -> Option<Url> {
    let text = typed.trim();
    if text.is_empty() {
        return None;
    }
    if text.starts_with('/') && !text.starts_with("//") {
        return Url::parse(&format!("file://{}", text)).ok();
    }
    if let Some(rest) = text.strip_prefix("~/") {
        let path = home_dir().join(rest);
        return Url::from_file_path(path).ok();
    }
    let low = text.to_lowercase();
    if low.contains("://")
        || low.starts_with("about:")
        || low.starts_with("data:")
        || low.starts_with("file:")
        || low.starts_with("search:")
        || low.starts_with("view-source:")
    {
        return Url::parse(text).ok();
    }
    let head = text.split(['/', '?', '#']).next().unwrap_or(text);
    let host = head.split(':').next().unwrap_or(head);
    let local = host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host == "127.0.0.1"
        || host == "0.0.0.0"
        || private_range(host)
        || host.starts_with("[::1]");
    Url::parse(&format!("{}://{}", if local { "http" } else { "https" }, text)).ok()
}

pub fn url_host(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
        .unwrap_or_default()
}

pub fn bare_host(url: &str) -> String {
    let host = url_host(url);
    host.strip_prefix("www.").unwrap_or(&host).to_string()
}

pub fn origin_of(url: &str) -> String {
    match Url::parse(url) {
        Ok(u) => {
            let scheme = u.scheme();
            match u.host_str() {
                Some(host) => {
                    let port = u.port().map(|p| format!(":{}", p)).unwrap_or_default();
                    format!("{}://{}{}", scheme, host, port)
                }
                None => format!("{}:", scheme),
            }
        }
        Err(_) => String::new(),
    }
}

/// The identity history is kept under: no fragment, hosts and schemes as
/// written in lower case, an empty path as a lone slash.
pub fn identity(url: &str) -> String {
    let Ok(mut parsed) = Url::parse(url) else {
        return url.to_string();
    };
    parsed.set_fragment(None);
    if (parsed.scheme() == "http" || parsed.scheme() == "https") && parsed.path().is_empty() {
        parsed.set_path("/");
    }
    parsed.to_string()
}

pub fn pretty_url(url: &str) -> String {
    let Ok(parsed) = Url::parse(url) else {
        return url.to_string();
    };
    if parsed.scheme() == "file" {
        if let Some(name) = parsed.path_segments().and_then(|mut s| s.next_back()) {
            return name.to_string();
        }
    }
    let Some(host) = parsed.host_str() else {
        return url.to_string();
    };
    let bare = host.strip_prefix("www.").unwrap_or(host);
    let path = parsed.path();
    if path.is_empty() || path == "/" {
        bare.to_string()
    } else {
        format!("{}{}", bare, path)
    }
}

pub fn editable_url(url: &str) -> String {
    let Ok(parsed) = Url::parse(url) else {
        return url.to_string();
    };
    if parsed.scheme() == "search" {
        return url.to_string();
    }
    if (parsed.scheme() == "http" || parsed.scheme() == "https") && parsed.host_str().is_some() {
        let mut text = parsed.host_str().unwrap_or("").to_string();
        if let Some(port) = parsed.port() {
            text.push(':');
            text.push_str(&port.to_string());
        }
        text.push_str(parsed.path());
        if let Some(q) = parsed.query() {
            text.push('?');
            text.push_str(q);
        }
        if text.ends_with('/') && parsed.path() == "/" {
            text.pop();
        }
        text
    } else {
        url.to_string()
    }
}

// ------------------------------------------------------------------ settings

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Prefs {
    /// The layout this copy of the app was last given. Existing settings
    /// from before the Search chrome was drawn (version 0) are moved to the
    /// column and the edge-to-edge page once; after that the switch is the
    /// reader's to keep wherever they leave it.
    pub ui_version: i32,
    pub look: String,
    pub sidebar: bool,
    pub auto_hide_sidebar: bool,
    pub side_position: String,
    pub side_width: i32,
    pub cards: bool,
    pub glyph: String,
    pub engine: String,
    pub custom_engine: String,
    pub shift_engine: String,
    pub shift_custom: String,
    pub adblock: bool,
    pub shield_paused: Vec<String>,
    pub darken_pages: bool,
    pub bookmarks_bar: bool,
    pub always_downloads: bool,
    pub downloads_dir: String,
    pub ask_downloads: bool,
    pub page_zoom: f64,
    pub saves_passwords: bool,
    pub fills_passwords: bool,
    pub static_bookmarks: bool,
    pub start_fresh: bool,
    pub compact_strip: bool,
    pub roomy_tabs: bool,
    pub side_icons: bool,
    pub reduce_motion: bool,
    pub show_links: bool,
    pub split_view: bool,
    pub restore_session: bool,
    pub confirm_quit: bool,
    pub developer_extras: bool,
    pub waits_for_play: bool,
    pub show_reading: bool,
    pub show_address: bool,
    pub autoscroll: bool,
    pub welcomes: bool,
    pub has_welcomed: bool,
    pub new_tab_place: i32,
    pub show_tab_shortcuts: bool,
    pub keeps_sites_awake: bool,
    pub sleep_tabs: bool,
    pub reader_follows_look: bool,
    pub engine_suggests: bool,
    pub keeps_signins: bool,
    pub keep_session_data: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            ui_version: 2,
            look: "system".into(),
            sidebar: true,
            auto_hide_sidebar: false,
            side_position: "left".into(),
            side_width: 232,
            cards: false,
            glyph: "letters".into(),
            engine: "google".into(),
            custom_engine: String::new(),
            shift_engine: String::new(),
            shift_custom: String::new(),
            adblock: true,
            shield_paused: Vec::new(),
            darken_pages: false,
            bookmarks_bar: false,
            always_downloads: false,
            downloads_dir: String::new(),
            ask_downloads: false,
            page_zoom: 1.0,
            saves_passwords: true,
            fills_passwords: true,
            static_bookmarks: false,
            start_fresh: false,
            compact_strip: false,
            roomy_tabs: false,
            side_icons: false,
            reduce_motion: false,
            show_links: true,
            split_view: true,
            restore_session: true,
            confirm_quit: false,
            developer_extras: false,
            waits_for_play: false,
            show_reading: true,
            show_address: true,
            autoscroll: false,
            welcomes: true,
            has_welcomed: false,
            new_tab_place: 1,
            show_tab_shortcuts: false,
            keeps_sites_awake: true,
            sleep_tabs: true,
            reader_follows_look: false,
            engine_suggests: false,
            keeps_signins: true,
            keep_session_data: true,
        }
    }
}

impl Prefs {
    pub fn path() -> PathBuf {
        config_dir().join("settings.json")
    }

    pub fn load() -> Self {
        let Some(text) = read_file(&Prefs::path()) else {
            return Prefs::default();
        };
        let mut prefs: Prefs = serde_json::from_str(&text).unwrap_or_default();
        if prefs.ui_version < 2 {
            // The interface now follows Search's own window: the column out,
            // the page edge to edge, the look and every other choice left
            // as they were.
            prefs.sidebar = true;
            prefs.cards = false;
            prefs.ui_version = 2;
            prefs.save();
        }
        if prefs.side_width < 176 || prefs.side_width > 440 {
            prefs.side_width = 232;
        }
        if prefs.downloads_dir.is_empty() {
            prefs.downloads_dir = downloads_dir().to_string_lossy().into_owned();
        }
        prefs
    }

    pub fn save(&self) {
        if let Ok(text) = serde_json::to_string_pretty(self) {
            write_atomic(&Prefs::path(), &text);
        }
    }

    pub fn downloads_path(&self) -> PathBuf {
        if self.downloads_dir.is_empty() {
            downloads_dir()
        } else {
            PathBuf::from(&self.downloads_dir)
        }
    }

    pub fn shield_paused_on(&self, host: &str) -> bool {
        let bare = host.strip_prefix("www.").unwrap_or(host);
        self.shield_paused.iter().any(|h| h == host || h == bare)
    }

    pub fn pause_shield(&mut self, host: &str, paused: bool) {
        let bare = host.strip_prefix("www.").unwrap_or(host).to_string();
        self.shield_paused.retain(|h| h != host && h != &bare);
        if paused {
            self.shield_paused.push(bare);
            self.shield_paused.sort();
        }
        self.save();
    }
}

// ------------------------------------------------------------------- history

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Visit {
    pub url: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub title: String,
    #[serde(default = "one")]
    pub count: i32,
    #[serde(default)]
    pub last: i64,
}

fn one() -> i32 {
    1
}

#[derive(Default, Debug, Clone)]
pub struct History {
    items: Vec<Visit>,
    /// Where each key lives in `items`, so recording a visit does not walk
    /// the whole list. Rebuilt whenever entries move.
    index: HashMap<String, usize>,
}

impl History {
    pub fn file() -> PathBuf {
        data_file("history.json")
    }

    pub fn load() -> Self {
        let Some(text) = read_file(&History::file()) else {
            return History::default();
        };
        let mut items: Vec<Visit> = serde_json::from_str(&text).unwrap_or_default();
        for item in &mut items {
            if item.key.is_empty() {
                item.key = identity(&item.url);
            }
            if item.last < 1_000_000_000 && item.last > 0 {
                // A session written by the Mac app counts seconds from 2001.
                item.last += 978_307_200;
            }
        }
        let mut history = History { items, index: HashMap::new() };
        history.reindex();
        history
    }

    /// The first entry wins, matching the linear search `visit` used before.
    fn reindex(&mut self) {
        self.index.clear();
        self.index.reserve(self.items.len());
        for (position, item) in self.items.iter().enumerate() {
            self.index.entry(item.key.clone()).or_insert(position);
        }
    }

    pub fn save(&self) {
        write_json_atomic(&History::file(), &self.items);
    }

    pub fn visit(&mut self, url: &str, title: &str) {
        if url.is_empty() || url.starts_with("search://") || url == "about:blank" {
            return;
        }
        let key = identity(url);
        let now = now_unix();
        let existing = self.index.get(&key).copied();
        if let Some(position) = existing {
            let item = &mut self.items[position];
            item.count = (item.count + 1).min(100_000);
            item.last = now;
            if !title.is_empty() {
                item.title = title.to_string();
            }
            item.url = url.to_string();
        } else {
            let position = self.items.len();
            self.items.push(Visit {
                url: url.to_string(),
                key: key.clone(),
                title: title.to_string(),
                count: 1,
                last: now,
            });
            self.index.insert(key, position);
            if self.items.len() > 20_000 {
                self.items.sort_by_key(|v| v.last);
                self.items.drain(0..1000);
                self.reindex();
            }
        }
        self.save();
    }

    pub fn remove(&mut self, url: &str) {
        let key = identity(url);
        self.items.retain(|v| v.key != key);
        self.reindex();
        self.save();
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.index.clear();
        self.save();
    }

    /// The newest `limit` visits, without sorting or cloning the rest of a
    /// list that can hold twenty thousand entries. Ties keep the order they
    /// have on disk, exactly as the stable full sort did.
    pub fn recent(&self, limit: usize) -> Vec<Visit> {
        if limit == 0 {
            return Vec::new();
        }
        if self.items.len() <= limit {
            let mut sorted = self.items.clone();
            sorted.sort_by_key(|visit| std::cmp::Reverse(visit.last));
            return sorted;
        }
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;
        // The root is whatever would fall just below the cut: the oldest
        // entry, and on a tie the latest in list order.
        let mut heap: BinaryHeap<(Reverse<i64>, usize)> = BinaryHeap::with_capacity(limit + 1);
        for (position, visit) in self.items.iter().enumerate() {
            heap.push((Reverse(visit.last), position));
            if heap.len() > limit {
                heap.pop();
            }
        }
        let mut picked: Vec<(i64, usize)> = heap
            .into_iter()
            .map(|(Reverse(last), position)| (last, position))
            .collect();
        picked.sort_by_key(|&(last, position)| (Reverse(last), position));
        picked
            .into_iter()
            .map(|(_, position)| self.items[position].clone())
            .collect()
    }

    pub fn all_sorted(&self) -> Vec<Visit> {
        self.recent(self.items.len().max(1))
    }

    pub fn empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn suggest(&self, typed: &str, limit: usize) -> Vec<Visit> {
        let typed_lower = typed.to_lowercase();
        let words: Vec<&str> = typed_lower.split_whitespace().collect();
        if words.is_empty() || limit == 0 {
            return Vec::new();
        }
        let now = now_unix();
        let mut scored: Vec<(f64, &Visit)> = self
            .items
            .iter()
            .filter_map(|visit| {
                // Lower-case the title once, and the address only when a word
                // was not already found in the title.
                let title = visit.title.to_lowercase();
                let mut url: Option<String> = None;
                for word in &words {
                    if title.contains(*word) {
                        continue;
                    }
                    let text = url.get_or_insert_with(|| editable_url(&visit.url).to_lowercase());
                    if !text.contains(*word) {
                        return None;
                    }
                }
                let days = ((now - visit.last).max(0) as f64) / 86_400.0;
                let mut score = visit.count as f64 * (-days / 30.0).exp();
                if title.starts_with(&typed_lower) {
                    score *= 2.0;
                }
                Some((score, visit))
            })
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(limit).map(|(_, v)| v.clone()).collect()
    }
}

// ----------------------------------------------------------------- bookmarks

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Bookmark {
    #[serde(default = "uuid4")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub children: Option<Vec<Bookmark>>,
}

impl Bookmark {
    pub fn site(title: &str, url: &str) -> Self {
        Bookmark {
            id: uuid4(),
            title: if title.trim().is_empty() {
                pretty_url(url)
            } else {
                title.to_string()
            },
            url: Some(url.to_string()),
            children: None,
        }
    }

    pub fn folder(title: &str) -> Self {
        Bookmark {
            id: uuid4(),
            title: title.to_string(),
            url: None,
            children: Some(Vec::new()),
        }
    }

    pub fn is_folder(&self) -> bool {
        self.url.is_none()
    }

    pub fn label(&self) -> String {
        if self.title.trim().is_empty() {
            self.url
                .as_deref()
                .map(pretty_url)
                .unwrap_or_else(|| "(untitled)".into())
        } else {
            self.title.clone()
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct Bookmarks {
    pub roots: Vec<Bookmark>,
}

#[derive(Clone, Debug)]
pub struct BookmarkHit {
    pub node: Bookmark,
    pub path: Vec<String>,
    pub depth: usize,
}

impl Bookmarks {
    pub fn file() -> PathBuf {
        data_file("bookmarks.json")
    }

    pub fn load() -> Self {
        let Some(text) = read_file(&Bookmarks::file()) else {
            return Bookmarks::default();
        };
        serde_json::from_str::<Vec<Bookmark>>(&text)
            .map(|roots| Bookmarks { roots })
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(text) = serde_json::to_string_pretty(&self.roots) {
            write_atomic(&Bookmarks::file(), &text);
        }
    }

    pub fn contains(&self, url: &str) -> bool {
        let key = identity(url);
        self.all().any(|b| {
            b.url
                .as_deref()
                .map(|u| identity(u) == key)
                .unwrap_or(false)
        })
    }

    pub fn toggle(&mut self, url: &str, title: &str) -> bool {
        let key = identity(url);
        let existing = self
            .all()
            .find(|b| {
                b.url
                    .as_deref()
                    .map(|u| identity(u) == key)
                    .unwrap_or(false)
            })
            .map(|b| b.id.clone());
        if let Some(id) = existing {
            self.remove(&id);
            false
        } else {
            self.roots.push(Bookmark::site(title, url));
            self.save();
            true
        }
    }

    pub fn add(&mut self, url: &str, title: &str) -> String {
        let node = Bookmark::site(title, url);
        let id = node.id.clone();
        self.roots.push(node);
        self.save();
        id
    }

    pub fn add_folder(&mut self, title: &str) -> String {
        let node = Bookmark::folder(title);
        let id = node.id.clone();
        self.roots.push(node);
        self.save();
        id
    }

    pub fn remove(&mut self, id: &str) -> bool {
        fn walk(nodes: &mut Vec<Bookmark>, id: &str) -> bool {
            if let Some(pos) = nodes.iter().position(|n| n.id == id) {
                nodes.remove(pos);
                return true;
            }
            for node in nodes.iter_mut() {
                if let Some(children) = node.children.as_mut() {
                    if walk(children, id) {
                        return true;
                    }
                }
            }
            false
        }
        let changed = walk(&mut self.roots, id);
        if changed {
            self.save();
        }
        changed
    }

    pub fn rename(&mut self, id: &str, title: &str) -> bool {
        fn walk(nodes: &mut [Bookmark], id: &str, title: &str) -> bool {
            for node in nodes.iter_mut() {
                if node.id == id {
                    node.title = title.to_string();
                    return true;
                }
                if let Some(children) = node.children.as_mut() {
                    if walk(children, id, title) {
                        return true;
                    }
                }
            }
            false
        }
        let changed = walk(&mut self.roots, id, title);
        if changed {
            self.save();
        }
        changed
    }

    pub fn all(&self) -> impl Iterator<Item = &Bookmark> {
        fn collect<'a>(nodes: &'a [Bookmark], out: &mut Vec<&'a Bookmark>) {
            for node in nodes {
                out.push(node);
                if let Some(children) = node.children.as_ref() {
                    collect(children, out);
                }
            }
        }
        let mut out = Vec::new();
        collect(&self.roots, &mut out);
        out.into_iter()
    }

    pub fn hits(&self) -> Vec<BookmarkHit> {
        fn walk(nodes: &[Bookmark], path: &mut Vec<String>, depth: usize, out: &mut Vec<BookmarkHit>) {
            for node in nodes {
                out.push(BookmarkHit {
                    node: node.clone(),
                    path: path.clone(),
                    depth,
                });
                if let Some(children) = node.children.as_ref() {
                    path.push(node.label());
                    walk(children, path, depth + 1, out);
                    path.pop();
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.roots, &mut Vec::new(), 0, &mut out);
        out
    }

    pub fn empty(&self) -> bool {
        self.roots.is_empty()
    }

    pub fn suggest(&self, typed: &str, limit: usize) -> Vec<Bookmark> {
        let typed_lower = typed.to_lowercase();
        let words: Vec<&str> = typed_lower.split_whitespace().collect();
        if words.is_empty() || limit == 0 {
            return Vec::new();
        }
        // Walk in the same order as `all` and stop as soon as the window is
        // full, instead of collecting every node first.
        fn walk(nodes: &[Bookmark], words: &[&str], limit: usize, out: &mut Vec<Bookmark>) {
            for node in nodes {
                if out.len() >= limit {
                    return;
                }
                if let Some(url) = node.url.as_deref() {
                    let title = node.title.to_lowercase();
                    let url = url.to_lowercase();
                    if words
                        .iter()
                        .all(|word| title.contains(*word) || url.contains(*word))
                    {
                        out.push(node.clone());
                    }
                }
                if let Some(children) = node.children.as_deref() {
                    walk(children, words, limit, out);
                }
            }
        }
        let mut out = Vec::with_capacity(limit.min(16));
        walk(&self.roots, &words, limit, &mut out);
        out
    }
}

// ------------------------------------------------------------------- session

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct SessionTab {
    pub url: String,
    pub title: String,
    pub pin: Option<String>,
    pub name: Option<String>,
    pub home: Option<String>,
    pub muted: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct SessionShape {
    pub tabs: Vec<SessionTab>,
    pub active: i32,
    pub splits: Vec<Value>,
}

pub fn session_file() -> PathBuf {
    data_file("session.json")
}

pub fn read_session() -> SessionShape {
    let Some(text) = read_file(&session_file()) else {
        return SessionShape::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn write_session(shape: &SessionShape) {
    if let Ok(text) = serde_json::to_string(shape) {
        write_atomic(&session_file(), &text);
    }
}

// -------------------------------------------------------------------- hidden

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Veil {
    pub selector: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub date: i64,
}

#[derive(Default, Debug, Clone)]
pub struct Curtain {
    by_host: HashMap<String, Vec<Veil>>,
}

pub fn curtain_host(url: &str) -> String {
    bare_host(url)
}

impl Curtain {
    pub fn file() -> PathBuf {
        data_file("hidden.json")
    }

    pub fn load() -> Self {
        let Some(text) = read_file(&Curtain::file()) else {
            return Curtain::default();
        };
        serde_json::from_str::<HashMap<String, Vec<Veil>>>(&text)
            .map(|by_host| Curtain { by_host })
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(text) = serde_json::to_string_pretty(&self.by_host) {
            write_atomic(&Curtain::file(), &text);
        }
    }

    pub fn veils(&self, host: &str) -> Vec<Veil> {
        self.by_host.get(host).cloned().unwrap_or_default()
    }

    pub fn all(&self) -> &HashMap<String, Vec<Veil>> {
        &self.by_host
    }

    pub fn hide(&mut self, host: &str, selector: &str, label: &str, note: &str) {
        let list = self.by_host.entry(host.to_string()).or_default();
        if list.iter().any(|v| v.selector == selector) {
            return;
        }
        list.push(Veil {
            selector: selector.to_string(),
            label: label.to_string(),
            note: note.to_string(),
            date: now_unix(),
        });
        self.save();
    }

    pub fn undo(&mut self, host: &str) -> Option<Veil> {
        let list = self.by_host.get_mut(host)?;
        let last = list.pop();
        if list.is_empty() {
            self.by_host.remove(host);
        }
        self.save();
        last
    }

    pub fn restore(&mut self, host: &str, selector: &str) {
        if let Some(list) = self.by_host.get_mut(host) {
            list.retain(|v| v.selector != selector);
            if list.is_empty() {
                self.by_host.remove(host);
            }
        }
        self.save();
    }

    pub fn restore_all(&mut self, host: &str) {
        self.by_host.remove(host);
        self.save();
    }

    pub fn css(&self, host: &str, spared: Option<&str>) -> String {
        self.veils(host)
            .into_iter()
            .filter(|v| Some(v.selector.as_str()) != spared)
            .map(|v| format!("{} {{ display: none !important; }}", v.selector))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ----------------------------------------------------------------- passwords

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Login {
    pub host: String,
    pub user: String,
    pub password: String,
    #[serde(default)]
    pub used: i64,
    #[serde(default)]
    pub clear: bool,
}

#[derive(Default, Debug, Clone)]
pub struct Vault {
    pub entries: Vec<Login>,
    pub trouble: Option<String>,
}

fn vault_key_path() -> PathBuf {
    data_dir().join("passwords.key")
}

fn vault_path() -> PathBuf {
    data_dir().join("passwords.enc")
}

impl Vault {
    fn key(&mut self, create: bool) -> Option<[u8; 32]> {
        let path = vault_key_path();
        if let Ok(bytes) = fs::read(&path) {
            if bytes.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                return Some(key);
            }
        }
        if !create {
            return None;
        }
        ensure_dir(&data_dir());
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        if !write_private(&path, &key) {
            self.trouble = Some("Couldn't write the password key".into());
            return None;
        }
        Some(key)
    }

    pub fn load() -> Self {
        let mut vault = Vault::default();
        let enc_path = vault_path();
        if !enc_path.exists() {
            return vault;
        }
        let Some(text) = read_file(&enc_path) else {
            vault.trouble = Some("Couldn't read saved passwords".into());
            return vault;
        };
        let Some(key) = vault.key(false) else {
            vault.trouble = Some("The password key is missing".into());
            return vault;
        };
        let decoded = match base64::engine::general_purpose::STANDARD.decode(text.trim()) {
            Ok(bytes) => bytes,
            Err(_) => {
                vault.trouble = Some("Saved passwords are damaged".into());
                return vault;
            }
        };
        if decoded.len() < 13 {
            vault.trouble = Some("Saved passwords are damaged".into());
            return vault;
        }
        let (nonce, ciphertext) = decoded.split_at(12);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        match cipher.decrypt(Nonce::from_slice(nonce), ciphertext) {
            Ok(plain) => match serde_json::from_slice::<Vec<Login>>(&plain) {
                Ok(entries) => vault.entries = entries,
                Err(_) => vault.trouble = Some("Saved passwords couldn't be read".into()),
            },
            Err(_) => vault.trouble = Some("Saved passwords couldn't be unlocked".into()),
        }
        vault
    }

    pub fn save(&mut self) {
        let Some(key) = self.key(true) else { return };
        let Ok(plain) = serde_json::to_vec(&self.entries) else {
            return;
        };
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let mut nonce = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce);
        let Ok(sealed) = cipher.encrypt(Nonce::from_slice(&nonce), plain.as_slice()) else {
            self.trouble = Some("Couldn't encrypt saved passwords".into());
            return;
        };
        let mut all = nonce.to_vec();
        all.extend_from_slice(&sealed);
        let text = base64::engine::general_purpose::STANDARD.encode(all);
        if write_private(&vault_path(), text.as_bytes()) {
            self.trouble = None;
        } else {
            self.trouble = Some("Couldn't write saved passwords".into());
        }
    }

    pub fn for_host(&self, host: &str) -> Vec<Login> {
        let bare = host.strip_prefix("www.").unwrap_or(host);
        let mut out: Vec<Login> = self
            .entries
            .iter()
            .filter(|login| {
                let kept = login.host.strip_prefix("www.").unwrap_or(&login.host);
                kept == bare || bare.ends_with(&format!(".{}", kept)) || kept.ends_with(&format!(".{}", bare))
            })
            .cloned()
            .collect();
        out.sort_by_key(|l| -l.used);
        out
    }

    pub fn any_for(&self, host: &str) -> bool {
        !self.for_host(host).is_empty()
    }

    pub fn keep(&mut self, host: &str, user: &str, password: &str, clear: bool) {
        if password.is_empty() {
            return;
        }
        let bare = host.strip_prefix("www.").unwrap_or(host).to_string();
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|l| l.host == bare && l.user == user)
        {
            existing.password = password.to_string();
            existing.used = now_unix();
            existing.clear = clear;
        } else {
            self.entries.push(Login {
                host: bare,
                user: user.to_string(),
                password: password.to_string(),
                used: now_unix(),
                clear,
            });
        }
        self.save();
    }

    pub fn forget(&mut self, host: &str, user: &str) {
        self.entries
            .retain(|l| !(l.host == host && l.user == user));
        self.save();
    }

    pub fn touch(&mut self, host: &str, user: &str) {
        if let Some(login) = self
            .entries
            .iter_mut()
            .find(|l| l.host == host && l.user == user)
        {
            login.used = now_unix();
        }
    }

    pub fn all_sorted(&self) -> Vec<Login> {
        let mut entries = self.entries.clone();
        entries.sort_by(|a, b| a.host.cmp(&b.host).then(a.user.cmp(&b.user)));
        entries
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

// -------------------------------------------------------------------- search

pub const ENGINES: [&str; 9] = [
    "google",
    "duckduckgo",
    "bing",
    "ecosia",
    "startpage",
    "kagi",
    "brave",
    "qwant",
    "custom",
];

pub fn engine_title(engine: &str) -> &'static str {
    match engine {
        "google" => "Google",
        "duckduckgo" => "DuckDuckGo",
        "bing" => "Bing",
        "ecosia" => "Ecosia",
        "startpage" => "Startpage",
        "kagi" => "Kagi",
        "brave" => "Brave Search",
        "qwant" => "Qwant",
        "custom" => "Custom",
        _ => "Google",
    }
}

pub fn engine_template(engine: &str, custom: &str) -> String {
    match engine {
        "google" => "https://www.google.com/search?q=%s".to_string(),
        "duckduckgo" => "https://duckduckgo.com/?q=%s".to_string(),
        "bing" => "https://www.bing.com/search?q=%s".to_string(),
        "ecosia" => "https://www.ecosia.org/search?q=%s".to_string(),
        "startpage" => "https://www.startpage.com/sp/search?query=%s".to_string(),
        "kagi" => "https://kagi.com/search?q=%s".to_string(),
        "brave" => "https://search.brave.com/search?q=%s".to_string(),
        "qwant" => "https://www.qwant.com/?q=%s".to_string(),
        "custom" => {
            let written = written_template(custom);
            if valid_template(&written) {
                written
            } else {
                "https://www.google.com/search?q=%s".to_string()
            }
        }
        _ => "https://www.google.com/search?q=%s".to_string(),
    }
}

fn written_template(custom: &str) -> String {
    let mut text = custom.trim().replace("{searchTerms}", "%s");
    if !text.is_empty() && !text.contains("://") {
        text = format!("https://{}", text);
    }
    text
}

fn valid_template(template: &str) -> bool {
    if !template.contains("%s") {
        return false;
    }
    let probe = template.replacen("%s", "a", 1);
    Url::parse(&probe)
        .ok()
        .and_then(|u| u.host_str().map(|h| !h.is_empty()))
        .unwrap_or(false)
}

pub fn engine_url(engine: &str, custom: &str, words: &str) -> Option<String> {
    let words = words.trim();
    if words.is_empty() {
        return None;
    }
    let template = engine_template(engine, custom);
    Some(template.replace("%s", &percent_encode(words)))
}

pub fn engine_name(engine: &str, custom: &str) -> String {
    if engine != "custom" {
        return engine_title(engine).to_string();
    }
    let template = written_template(custom);
    Url::parse(&template)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.strip_prefix("www.").unwrap_or(h).to_string()))
        .unwrap_or_else(|| "Google".into())
}

// --------------------------------------------------------------------- about

pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn all_logins_sorted(vault: &Vault) -> Vec<Login> {
    vault.all_sorted()
}

pub fn sorted_hosts(curtain: &Curtain) -> Vec<String> {
    let mut hosts: Vec<String> = curtain.all().keys().cloned().collect();
    hosts.sort();
    hosts
}

pub fn unique_hosts_of(history: &History) -> Vec<String> {
    let mut seen = HashSet::new();
    for item in &history.items {
        let host = bare_host(&item.url);
        if !host.is_empty() {
            seen.insert(host);
        }
    }
    let mut list: Vec<String> = seen.into_iter().collect();
    list.sort();
    list
}

pub fn parse_json(text: &str) -> Option<Value> {
    serde_json::from_str(text).ok()
}

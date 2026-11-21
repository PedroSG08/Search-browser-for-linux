// The pages Search draws for itself: settings, history, bookmarks, downloads,
// passwords, what is hidden on a site, the welcome, and the blank new tab.
// They are ordinary pages in the same WebKit view as the web, so the address
// field, Back and Find work there as everywhere else.

use serde::Serialize;
use serde_json::json;

use crate::store::{
    self, app_version, Bookmarks, Curtain, History, Login, Prefs, Vault,
};
use crate::util::html;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageKind {
    Blank,
    Web,
    Settings,
    History,
    Bookmarks,
    Downloads,
    Passwords,
    Welcome,
    Hidden,
}

impl PageKind {
    pub fn internal(self) -> bool {
        !matches!(self, PageKind::Web | PageKind::Blank)
    }

    pub fn name(self) -> &'static str {
        match self {
            PageKind::Blank => "blank",
            PageKind::Web => "web",
            PageKind::Settings => "settings",
            PageKind::History => "history",
            PageKind::Bookmarks => "bookmarks",
            PageKind::Downloads => "downloads",
            PageKind::Passwords => "passwords",
            PageKind::Welcome => "welcome",
            PageKind::Hidden => "hidden",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            PageKind::Blank => "New Tab",
            PageKind::Web => "",
            PageKind::Settings => "Settings",
            PageKind::History => "History",
            PageKind::Bookmarks => "Bookmarks",
            PageKind::Downloads => "Downloads",
            PageKind::Passwords => "Passwords",
            PageKind::Welcome => "Welcome",
            PageKind::Hidden => "Hidden on This Site",
        }
    }
}

pub fn page_kind_for_url(url: &str) -> Option<PageKind> {
    let parsed = url::Url::parse(url).ok()?;
    if parsed.scheme() != "search" {
        return None;
    }
    match parsed.host_str().unwrap_or("") {
        "blank" => Some(PageKind::Blank),
        "settings" => Some(PageKind::Settings),
        "history" => Some(PageKind::History),
        "bookmarks" => Some(PageKind::Bookmarks),
        "downloads" => Some(PageKind::Downloads),
        "passwords" => Some(PageKind::Passwords),
        "welcome" => Some(PageKind::Welcome),
        "hidden" => Some(PageKind::Hidden),
        _ => None,
    }
}

pub fn internal_url(kind: PageKind) -> String {
    format!("search://{}", kind.name())
}

#[derive(Serialize, Clone, Debug)]
pub struct DownloadView {
    pub index: usize,
    pub name: String,
    pub path: String,
    pub host: String,
    #[serde(default)]
    pub url: String,
    pub received: u64,
    pub total: u64,
    pub state: String,
}

// --------------------------------------------------------------------- shell

fn base_css() -> &'static str {
    r#"
:root {
  --ground:#ffffff; --ink:#171717; --muted:#8a8a8a; --faint:#d4d4d4;
  --hairline:#e8e8e8; --wash:#efefef; --frame:#f3f3f3; --tile:#ececec;
  --accent:#171717; --accent-ink:#ffffff; --safe:#147f3f; --danger:#b83a2e;
  color-scheme: light;
}
:root[data-look="dark"], :root[data-system-dark="1"] {
  --ground:#1c1c1c; --ink:#ededed; --muted:#949494; --faint:#525252;
  --hairline:#333333; --wash:#2d2d2d; --frame:#131313; --tile:#1d1d1d;
  --accent:#ededed; --accent-ink:#1c1c1c; --safe:#4ade80; --danger:#f87171;
  color-scheme: dark;
}
* { box-sizing: border-box; }
html, body { margin:0; padding:0; background:var(--ground); color:var(--ink); }
body {
  font: 13px/1.5 "Cantarell","Inter",-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;
  -webkit-font-smoothing: antialiased;
}
a { color:inherit; }
button, select, input {
  font:inherit; color:var(--ink); background:var(--ground);
}
button { cursor:pointer; }
.shell { max-width: 820px; margin:0 auto; padding:52px 28px 90px; }
.narrow { max-width: 660px; }
h1 { font-size:26px; line-height:1.2; letter-spacing:-.02em; font-weight:650; margin:0 0 6px; }
h2 { font-size:15px; font-weight:650; letter-spacing:-.01em; margin:34px 0 8px; }
h3 { font-size:13px; font-weight:650; margin:0 0 2px; }
p { margin:0 0 14px; }
.sub { color:var(--muted); font-size:12.5px; }
.mono { font-family:ui-monospace,"Cantarell Mono",monospace; font-size:11.5px; }
.hairline { height:1px; background:var(--hairline); border:0; margin:18px 0; }
.row { display:flex; align-items:center; gap:12px; padding:11px 2px; border-bottom:1px solid var(--hairline); }
.row:last-child { border-bottom:0; }
.row .grow { flex:1; min-width:0; }
.row .title { font-size:13px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.row .detail { font-size:11.5px; color:var(--muted); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.row .actions { display:flex; align-items:center; gap:6px; }
.icon {
  width:26px; height:26px; border-radius:7px; background:var(--tile); color:var(--muted);
  display:flex; align-items:center; justify-content:center; font-weight:650; font-size:12px;
  flex:none; text-transform:uppercase;
}
.toolbar { display:flex; align-items:center; gap:10px; margin:22px 0 8px; }
.toolbar .grow { flex:1; }
.search {
  flex:1; border:1px solid var(--hairline); background:var(--ground); border-radius:9px;
  padding:8px 11px; outline:none; min-width:80px;
}
.search:focus { border-color:var(--muted); }
.pill {
  border:1px solid var(--hairline); background:var(--ground); border-radius:8px;
  padding:6px 11px; font-size:12px;
}
.pill:hover { background:var(--wash); }
.pill.plain { border-color:transparent; color:var(--muted); }
.pill.plain:hover { color:var(--ink); background:var(--wash); }
.pill.danger { color:var(--danger); }
.pill.accent { background:var(--accent); color:var(--accent-ink); border-color:var(--accent); font-weight:600; }
.pill.accent:hover { opacity:.88; }
.empty { color:var(--muted); text-align:center; padding:48px 10px; font-size:13px; }
.list { margin-top:8px; }
.list .row { padding:10px 6px; border-radius:8px; }
.list .row:hover { background:var(--wash); }
.section-note { color:var(--muted); font-size:11.5px; margin:4px 0 16px; }
.kbd {
  display:inline-block; min-width:17px; text-align:center; border:1px solid var(--hairline);
  background:var(--tile); border-radius:4px; padding:0 4px; font-size:10.5px; color:var(--muted);
}
.mark { width:74px; height:auto; display:block; margin-bottom:22px; }
.welcome { min-height:100vh; display:flex; align-items:center; justify-content:center; padding:40px; }
.welcome .card { width:min(560px,100%); }
.welcome h1 { font-size:38px; margin-top:0; }
.welcome p { font-size:15px; line-height:1.6; color:var(--muted); }
.footer { margin-top:54px; color:var(--muted); font-size:11px; }
.settings-wrap { display:flex; min-height:100vh; }
.settings-nav {
  width:190px; flex:none; border-right:1px solid var(--hairline); padding:26px 12px;
  position:sticky; top:0; height:100vh; overflow:auto;
}
.settings-nav button {
  display:flex; width:100%; text-align:left; border:0; background:transparent;
  color:var(--muted); border-radius:7px; padding:7px 10px; margin:1px 0; font-size:12.5px;
}
.settings-nav button:hover { background:var(--wash); color:var(--ink); }
.settings-nav button.active { background:var(--wash); color:var(--ink); font-weight:600; }
.settings-main { flex:1; min-width:0; padding:40px 42px 100px; max-width:760px; }
.panel { display:none; }
.panel.active { display:block; }
.panel h1 { margin-bottom:24px; }
.set-row {
  display:flex; gap:20px; align-items:center; justify-content:space-between;
  padding:12px 0; border-bottom:1px solid var(--hairline);
}
.set-row:last-child { border-bottom:0; }
.set-row .label { flex:1; min-width:0; }
.set-row .label b { display:block; font-size:13px; font-weight:500; }
.set-row .label span { display:block; font-size:11.5px; color:var(--muted); margin-top:2px; max-width:430px; }
.set-row .control { flex:none; display:flex; align-items:center; gap:6px; }
.switch { position:relative; width:36px; height:21px; flex:none; }
.switch input { opacity:0; width:0; height:0; }
.switch i {
  position:absolute; inset:0; background:var(--faint); border-radius:21px; transition:background .13s;
}
.switch i:before {
  content:""; position:absolute; width:17px; height:17px; left:2px; top:2px; background:#fff;
  border-radius:50%; box-shadow:0 1px 2px rgba(0,0,0,.15); transition:transform .13s;
}
.switch input:checked + i { background:var(--accent); }
.switch input:checked + i:before { transform:translateX(15px); background:var(--accent-ink); }
.seg { display:flex; border:1px solid var(--hairline); border-radius:8px; overflow:hidden; }
.seg button { border:0; background:transparent; padding:5px 10px; font-size:12px; color:var(--muted); }
.seg button + button { border-left:1px solid var(--hairline); }
.seg button.selected { background:var(--wash); color:var(--ink); font-weight:600; }
select {
  border:1px solid var(--hairline); border-radius:8px; padding:6px 9px; background:var(--ground);
  outline:none; max-width:220px;
}
input.text {
  border:1px solid var(--hairline); border-radius:8px; padding:6px 9px; background:var(--ground);
  outline:none; width:240px;
}
.folder { margin-left:20px; border-left:1px solid var(--hairline); padding-left:10px; }
.folder-title { display:flex; align-items:center; gap:6px; padding:9px 4px; color:var(--muted); font-size:11.5px; }
.progress { height:4px; background:var(--wash); border-radius:3px; overflow:hidden; margin-top:5px; }
.progress i { display:block; height:100%; background:var(--accent); }
.pw-reveal { font-family:ui-monospace,monospace; color:var(--muted); }
.badge {
  display:inline-block; font-size:10px; font-weight:600; color:var(--muted);
  background:var(--wash); border-radius:5px; padding:1px 6px; margin-left:6px;
}
@media (max-width:640px) {
  .settings-nav { width:150px; padding:18px 8px; }
  .settings-main { padding:26px 22px 80px; }
  .shell { padding:34px 18px 70px; }
}
"#
}

fn logo_svg() -> &'static str {
    r#"<svg class="mark" viewBox="0 0 608 276" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><path d="M469.443 0C545.471 0.00013198 607.103 61.6325 607.104 137.66C607.104 213.688 545.471 275.321 469.443 275.321H137.66C61.6323 275.321 0 213.688 0 137.66C0.00016085 61.6325 61.6325 0.000140192 137.66 0H469.443ZM138.104 51.5977C127.234 51.5977 117.512 53.5115 108.938 57.3389C100.518 61.0132 93.8581 66.2188 88.959 72.9551C84.2132 79.5381 81.8398 87.3464 81.8398 96.3789C81.8399 105.258 83.6773 112.607 87.3516 118.425C91.0258 124.089 95.9251 128.682 102.049 132.203C108.173 135.571 114.833 138.327 122.028 140.471L151.652 149.197C158.389 151.188 163.9 154.249 168.187 158.383C172.473 162.516 174.617 168.028 174.617 174.917C174.617 182.572 171.402 188.849 164.972 193.748C158.695 198.494 150.122 200.867 139.252 200.867C132.21 200.867 125.702 199.413 119.731 196.504C113.914 193.442 109.091 189.308 105.264 184.103C101.436 178.744 99.2169 172.697 98.6045 165.961H97.6855L75.4102 171.013C76.3287 180.658 79.697 189.308 85.5146 196.963C91.3322 204.618 98.9103 210.665 108.249 215.104C117.741 219.544 128.076 221.765 139.252 221.765C151.193 221.765 161.68 219.774 170.713 215.794C179.746 211.813 186.711 206.225 191.61 199.029C196.662 191.834 199.188 183.414 199.188 173.769C199.188 164.124 197.352 156.239 193.678 150.115C190.003 143.838 185.104 138.863 178.98 135.188C172.857 131.514 166.044 128.605 158.542 126.462L128.229 117.735C121.799 115.898 116.516 113.219 112.383 109.698C108.402 106.177 106.412 101.354 106.412 95.2305C106.412 88.188 109.168 82.6758 114.68 78.6953C120.344 74.5619 128.152 72.4951 138.104 72.4951C147.901 72.4952 155.939 74.9448 162.216 79.8438C168.493 84.7428 172.397 91.1729 173.928 99.1338H174.847L196.663 93.8525C195.745 85.5853 192.605 78.3131 187.247 72.0361C181.889 65.6061 174.923 60.6306 166.35 57.1094C157.929 53.4351 148.514 51.5977 138.104 51.5977Z" fill="currentColor"/></svg>"#
}

fn shell(kind: PageKind, title: &str, body: &str, extra_css: &str, script: &str, look: &str) -> String {
    let look_attr = match look {
        "light" | "dark" => format!(" data-look=\"{}\"", look),
        _ if crate::util::system_prefers_dark() => " data-system-dark=\"1\"".to_string(),
        _ => String::new(),
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="en"{look_attr}>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="light dark">
<title>{title}</title>
<style>{base}{extra}</style>
</head>
<body data-page="{page}">
{body}
<script>
const SEARCH_PAGE = {page_json};
function post(object) {{
  try {{ window.webkit.messageHandlers.search.postMessage(JSON.stringify(object)); }} catch (e) {{}}
}}
document.addEventListener('click', function (event) {{
  const target = event.target.closest('[data-action]');
  if (!target) return;
  const data = {{ action: target.dataset.action }};
  for (const key of ['url','id','page','host','user','value','name','path']) {{
    if (target.dataset[key] !== undefined) data[key] = target.dataset[key];
  }}
  if (target.dataset.action === 'setting' || target.dataset.action === 'choice') {{
    data.key = target.dataset.key;
    data.value = target.dataset.value;
    if (target.dataset.choice !== undefined) data.value = target.dataset.choice;
  }}
  event.preventDefault();
  post(data);
}});
document.addEventListener('change', function (event) {{
  const el = event.target;
  if (el.matches('[data-setting]')) {{
    post({{action:'setting', key: el.dataset.setting, value: el.type === 'checkbox' ? el.checked : el.value}});
  }}
}});
document.addEventListener('keydown', function (event) {{
  if (event.key === 'Escape') post({{action:'escape'}});
}});
{script}
</script>
</body>
</html>"#,
        look_attr = look_attr,
        title = html(title),
        base = base_css(),
        extra = extra_css,
        page = kind.name(),
        page_json = serde_json::to_string(kind.name()).unwrap_or_else(|_| "\"\"".into()),
        body = body,
        script = script
    )
}

fn ch(href: &str, label: &str, active: bool) -> String {
    format!(
        "<button data-action=\"go\" data-page=\"{}\" class=\"{}\">{}</button>",
        html(href),
        if active { "active" } else { "" },
        html(label)
    )
}

// ------------------------------------------------------------------ settings

pub fn settings(prefs: &Prefs, vault: &Vault, current: &str) -> String {
    let tabs = ["general", "appearance", "tabs", "browsing", "privacy", "passwords", "about"];
    let nav = tabs
        .iter()
        .map(|t| {
            let label = match *t {
                "general" => "General",
                "appearance" => "Appearance",
                "tabs" => "Tabs",
                "browsing" => "Browsing",
                "privacy" => "Privacy",
                "passwords" => "Passwords",
                "about" => "About",
                _ => t,
            };
            ch(t, label, current == *t)
        })
        .collect::<Vec<_>>()
        .join("");

    let engines = store::ENGINES
        .iter()
        .map(|e| {
            format!(
                "<option value=\"{}\"{}>{}</option>",
                e,
                if prefs.engine == *e { " selected" } else { "" },
                html(store::engine_title(e))
            )
        })
        .collect::<String>();

    let switch = |key: &str, on: bool| {
        format!(
            "<label class=\"switch\"><input type=\"checkbox\" data-setting=\"{}\"{}><i></i></label>",
            key,
            if on { " checked" } else { "" }
        )
    };
    let seg = |key: &str, choices: &[(&str, &str)], selected: &str| {
        let mut out = String::from("<div class=\"seg\">");
        for (value, label) in choices {
            out.push_str(&format!(
                "<button data-action=\"choice\" data-key=\"{}\" data-choice=\"{}\" class=\"{}\">{}</button>",
                key,
                value,
                if selected == *value { "selected" } else { "" },
                html(label)
            ));
        }
        out.push_str("</div>");
        out
    };
    let set_row = |title: &str, note: &str, control: String| {
        format!(
            "<div class=\"set-row\"><div class=\"label\"><b>{}</b><span>{}</span></div><div class=\"control\">{}</div></div>",
            html(title),
            html(note),
            control
        )
    };

    let general = format!(
        r#"<section class="panel {}" id="panel-general">
<h1>General</h1>
<div class="set-row"><div class="label"><b>Search engine</b><span>Where words typed in the address field are sent.</span></div>
<div class="control"><select data-setting="engine">{engines}</select></div></div>
<div class="set-row"><div class="label"><b>Custom search address</b><span>With %s where the words go. Used when Custom is chosen.</span></div>
<div class="control"><input class="text" data-setting="custom_engine" value="{custom}" placeholder="https://example.com/?q=%s"></div></div>
{downloads_row}
{ask_downloads}
{restore_session}
{start_fresh}
{confirm_quit}
<div class="footer">Search for Linux · version {version}</div>
</section>"#,
        if current == "general" { "active" } else { "" },
        engines = engines,
        custom = html(&prefs.custom_engine),
        downloads_row = set_row(
            "Downloads",
            &format!("Saved here unless you choose somewhere else. Currently {}.", prefs.downloads_path().display()),
            "<button class=\"pill\" data-action=\"choose-downloads\">Change…</button>".into()
        ),
        ask_downloads = set_row("Ask where to save", "Choose a folder and a name for each download.", switch("ask_downloads", prefs.ask_downloads)),
        restore_session = set_row("Restore tabs at launch", "Open the pages that were open when Search last quit.", switch("restore_session", prefs.restore_session)),
        start_fresh = set_row("Start with a fresh window", "Keep pins and named tabs, and leave the rest of last session behind.", switch("start_fresh", prefs.start_fresh)),
        confirm_quit = set_row("Confirm before quitting", "Ask once before closing a window with several tabs.", switch("confirm_quit", prefs.confirm_quit)),
        version = app_version()
    );

    let appearance = format!(
        r#"<section class="panel {}" id="panel-appearance">
<h1>Appearance</h1>
{look}
{cards}
{bookmarks_bar}
{compact}
{roomy}
{glyph}
{reduce_motion}
</section>"#,
        if current == "appearance" { "active" } else { "" },
        look = set_row("Look", "Light, dark, or whatever the desktop is doing.", seg("look", &[("light", "Light"), ("dark", "Dark"), ("system", "System")], &prefs.look)),
        cards = set_row("Page as a card", "A small gap and rounded edge around the page, with tabs down the side.", switch("cards", prefs.cards)),
        bookmarks_bar = set_row("Bookmarks bar", "A quiet row of what you kept, under the tabs.", switch("bookmarks_bar", prefs.bookmarks_bar)),
        compact = set_row("Compact tab strip", "A thinner row when the tabs are across the top.", switch("compact_strip", prefs.compact_strip)),
        roomy = set_row("Roomy tabs", "More air around each row in the sidebar.", switch("roomy_tabs", prefs.roomy_tabs)),
        glyph = set_row("Tabs wear", "A letter from the page, or its icon.", seg("glyph", &[("letters", "Letters"), ("icons", "Icons")], &prefs.glyph)),
        reduce_motion = set_row("Reduce animations", "Put things where they are going at once.", switch("reduce_motion", prefs.reduce_motion)),
    );

    let new_place = format!(
        "<select data-setting=\"new_tab_place\">\
         <option value=\"0\"{}>Next to this one</option>\
         <option value=\"1\"{}>At the end</option>\
         <option value=\"2\"{}>At the top</option></select>",
        if prefs.new_tab_place == 0 { " selected" } else { "" },
        if prefs.new_tab_place == 1 { " selected" } else { "" },
        if prefs.new_tab_place == 2 { " selected" } else { "" },
    );
    let tabs = format!(
        r#"<section class="panel {}" id="panel-tabs">
<h1>Tabs</h1>
{sidebar}
{auto_hide}
{side_right}
{split}
{new_place}
{shortcuts}
{reader}
{keep_awake}
{sleep}
</section>"#,
        if current == "tabs" { "active" } else { "" },
        sidebar = set_row("Tabs in the sidebar", "Down the left instead of across the top.", switch("sidebar", prefs.sidebar)),
        auto_hide = set_row("Auto-hide sidebar", "Hide the sidebar; it slides in when the pointer reaches the left edge.", switch("auto_hide_sidebar", prefs.auto_hide_sidebar)),
        side_right = set_row("Sidebar on the right", "Tabs down the right edge instead.", switch("side_position_right", prefs.side_position == "right")),
        split = set_row("Split View", "Two pages side by side. Ctrl+Alt+N splits the current page.", switch("split_view", prefs.split_view)),
        new_place = set_row("New tabs open", "Where a new page lands in the row.", new_place),
        shortcuts = set_row("Show shortcut numbers", "Number pins and tabs as Ctrl+1–9 would pick them.", switch("show_tab_shortcuts", prefs.show_tab_shortcuts)),
        reader = set_row("Reading mode", "Show the reading command in the tools menu.", switch("show_reading", prefs.show_reading)),
        keep_awake = set_row("Keep selected sites awake", "Allow a site to ask not to be slept or closed. Kept per tab.", switch("keeps_sites_awake", prefs.keeps_sites_awake)),
        sleep = set_row("Put inactive tabs to sleep", "Free their memory after ten minutes without use. A sleeping tab reloads when you return; pinned tabs and anything playing audio stay awake.", switch("sleep_tabs", prefs.sleep_tabs)),
    );

    let browsing = format!(
        r#"<section class="panel {}" id="panel-browsing">
<h1>Browsing</h1>
{adblock}
{darken}
{reader_follows}
{waits}
{links}
{zoom}
{dev}
</section>"#,
        if current == "browsing" { "active" } else { "" },
        adblock = set_row("uBlock filters", "Official uBlock Origin lists (EasyList, EasyPrivacy and more) are applied before the page is drawn.", switch("adblock", prefs.adblock)),
        darken = set_row("Darken pages", "Turn a site's own white page dark. Pictures are left alone.", switch("darken_pages", prefs.darken_pages)),
        reader_follows = set_row("Reading mode follows the look", "Dark reading pages in a dark window.", switch("reader_follows_look", prefs.reader_follows_look)),
        waits = set_row("Wait for play", "Pages may not start video with sound on their own.", switch("waits_for_play", prefs.waits_for_play)),
        links = set_row("Show where a link goes", "A small address at the bottom while the pointer rests on a link.", switch("show_links", prefs.show_links)),
        zoom = set_row("Page zoom", "The size of every page. Ctrl+= and Ctrl+- change it as you go.", format!("<span class=\"sub\">{:.0}%</span>", prefs.page_zoom * 100.0)),
        dev = set_row("Developer extras", "Show the Web Inspector in the tools menu.", switch("developer_extras", prefs.developer_extras)),
    );

    let paused = if prefs.shield_paused.is_empty() {
        String::new()
    } else {
        let rows = prefs
            .shield_paused
            .iter()
            .map(|host| {
                format!(
                    "<div class=\"row\"><div class=\"grow\"><div class=\"title\">{}</div><div class=\"detail\">uBlock filters are paused here.</div></div><div class=\"actions\"><button class=\"pill\" data-action=\"shield-resume\" data-host=\"{}\">Turn back on</button></div></div>",
                    html(host), html(host)
                )
            })
            .collect::<String>();
        format!("<h2>Paused on</h2><div class=\"list\">{}</div>", rows)
    };
    let trouble = vault
        .trouble
        .as_ref()
        .map(|t| format!("<p class=\"sub\" style=\"color:var(--danger)\">{}</p>", html(t)))
        .unwrap_or_default();
    let privacy = format!(
        r#"<section class="panel {}" id="panel-privacy">
<h1>Privacy</h1>
{saves}
{fills}
{signins}
{keep_session}
{shield}
{paused}
<h2>Clear</h2>
<div class="row"><div class="grow"><div class="title">Browsing history</div><div class="detail">Every address Search remembers, gone.</div></div><div class="actions"><button class="pill danger" data-action="clear-history">Clear</button></div></div>
<div class="row"><div class="grow"><div class="title">Cookies and site data</div><div class="detail">Sign out of every site and forget what they stored.</div></div><div class="actions"><button class="pill danger" data-action="clear-sites">Sign out of everything</button></div></div>
<div class="row"><div class="grow"><div class="title">Cache</div><div class="detail">Fetched files kept to make pages quicker.</div></div><div class="actions"><button class="pill" data-action="clear-cache">Clear</button></div></div>
<div class="row"><div class="grow"><div class="title">Hidden elements</div><div class="detail">Everything taken off pages, for every site.</div></div><div class="actions"><button class="pill" data-action="go" data-page="hidden">Review…</button></div></div>
<div class="footer">Nothing is sent anywhere. Your pages, history, bookmarks and passwords stay on this computer.</div>
</section>"#,
        if current == "privacy" { "active" } else { "" },
        saves = set_row("Save passwords", "Offer to keep a sign-in once it has worked.", switch("saves_passwords", prefs.saves_passwords)),
        fills = set_row("Fill passwords", "Offer saved accounts under the address field. Never without a click.", switch("fills_passwords", prefs.fills_passwords)),
        signins = set_row("Keep sign-ins", "Keep cookies and site storage between launches.", switch("keeps_signins", prefs.keeps_signins)),
        keep_session = set_row("Keep session data on close", "Keep cookies, site data and signed-in accounts when Search closes. Passwords saved in Passwords are kept separately.", switch("keep_session_data", prefs.keep_session_data)),
        shield = set_row("uBlock filters", "The official uBlock Origin lists, applied before the page is drawn.", switch("adblock", prefs.adblock)),
        paused = paused,
    );

    let passwords = format!(
        r#"<section class="panel {}" id="panel-passwords">
<h1>Passwords</h1>
{trouble}
<p class="sub">{count} sign-in{plural} kept, encrypted with AES-256-GCM and a key only your user can read.</p>
<div class="toolbar"><div class="grow"></div><button class="pill accent" data-action="go" data-page="passwords">Open Passwords…</button></div>
{saves}
{fills}
</section>"#,
        if current == "passwords" { "active" } else { "" },
        trouble = trouble,
        count = vault.count(),
        plural = if vault.count() == 1 { "" } else { "s" },
        saves = set_row("Save passwords", "Offer to keep a sign-in once it has worked.", switch("saves_passwords", prefs.saves_passwords)),
        fills = set_row("Fill passwords", "Offer saved accounts under the address field. Never without a click.", switch("fills_passwords", prefs.fills_passwords)),
    );

    let about = format!(
        r#"<section class="panel {}" id="panel-about">
<h1>About</h1>
{logo}
<p><b>Search</b> is a small, quiet web browser. This is the Linux edition, drawn with GTK and running on WebKitGTK — the same engine family the Mac app uses.</p>
<p class="sub">Version {version} · WebKitGTK</p>
<h2>Extensions and blocking</h2>
<p class="sub">WebKitGTK does not load Chrome or Firefox extensions, so uBlock Origin cannot be installed as an extension here. Instead, Search compiles uBlock Origin's official filter lists into WebKit's native content blocker, enabled by default. Pause it per site from the shield in the address bar.</p>
<h2>Keyboard</h2>
<div class="list">
{keys}
</div>
<h2>What is kept, and where</h2>
<p class="sub">Settings live in <span class="mono">~/.config/Search</span>; history, bookmarks, open tabs and hidden elements in <span class="mono">~/.local/share/Search</span>. Passwords are encrypted there with a key beside them, readable only by your user. There is no account, no sync, no telemetry and no server.</p>
<div class="footer">MIT licensed. “Search” and its mark are Office Commun's; this port is for Linux.</div>
</section>"#,
        if current == "about" { "active" } else { "" },
        logo = logo_svg(),
        version = app_version(),
        keys = [
            ("Ctrl L", "address field"),
            ("Ctrl K", "address field"),
            ("Ctrl T", "new tab"),
            ("Ctrl Shift N", "private tab"),
            ("Ctrl W", "close tab"),
            ("Ctrl F", "find on page"),
            ("Ctrl Shift R", "reading mode"),
            ("Ctrl Shift H", "hide something"),
            ("Ctrl Shift S", "sidebar or tabs on top"),
            ("Ctrl S", "fold the sidebar"),
            ("Ctrl 1–9", "jump to a tab"),
        ]
        .iter()
        .map(|(key, what)| {
            format!(
                "<div class=\"row\"><div class=\"grow\"><div class=\"title\">{}</div></div><span class=\"kbd\">{}</span></div>",
                html(what),
                html(key)
            )
        })
        .collect::<String>(),
    );

    let body = format!(
        "<div class=\"settings-wrap\"><nav class=\"settings-nav\">{}</nav><main class=\"settings-main\">{}{}{}{}{}{}{}</main></div>",
        nav, general, appearance, tabs, browsing, privacy, passwords, about
    );
    shell(
        PageKind::Settings,
        "Settings",
        &body,
        "",
        r#"
const buttons = document.querySelectorAll('.settings-nav button');
buttons.forEach(function (button) {
  button.addEventListener('click', function (event) {
    event.preventDefault();
    event.stopPropagation();
    buttons.forEach(function (other) { other.classList.toggle('active', other === button); });
    document.querySelectorAll('.panel').forEach(function (panel) {
      panel.classList.toggle('active', panel.id === 'panel-' + button.dataset.page);
    });
    window.scrollTo(0, 0);
  });
});
document.querySelectorAll('.seg button').forEach(function (button) {
  button.addEventListener('click', function () {
    if (button.dataset.key !== undefined) {
      button.parentElement.querySelectorAll('button').forEach(function (other) {
        other.classList.toggle('selected', other === button);
      });
    }
  });
});
"#,
        &prefs.look,
    )
}

// ------------------------------------------------------------------- history

pub fn history(history: &History, prefs: &Prefs) -> String {
    let items = history.recent(500);
    let rows = if items.is_empty() {
        "<div class=\"empty\">Nothing here yet. Where you go will be kept here, and nowhere else.</div>".into()
    } else {
        items
            .iter()
            .map(|visit| {
                format!(
                    r#"<div class="row history-row" data-search="{search}">
  <div class="icon">{letter}</div>
  <div class="grow"><div class="title">{title}</div><div class="detail">{url} · {when}</div></div>
  <div class="actions">
    <button class="pill plain" data-action="open" data-url="{url_attr}">Open</button>
    <button class="pill plain" data-action="history-delete" data-url="{url_attr}" title="Forget this visit">Forget</button>
  </div>
</div>"#,
                    search = html(&format!("{} {}", visit.title, visit.url).to_lowercase()),
                    letter = html(
                        &visit
                            .title
                            .chars()
                            .next()
                            .or_else(|| store::bare_host(&visit.url).chars().next())
                            .unwrap_or('•')
                            .to_string()
                    ),
                    title = html(&if visit.title.is_empty() { store::pretty_url(&visit.url) } else { visit.title.clone() }),
                    url = html(&store::pretty_url(&visit.url)),
                    url_attr = html(&visit.url),
                    when = html(&store::time_ago(visit.last))
                )
            })
            .collect::<String>()
    };
    let body = format!(
        r#"<div class="shell narrow">
<h1>History</h1>
<p class="sub">Where you have been, kept on this computer only.</p>
<div class="toolbar">
  <input class="search" id="filter" type="search" placeholder="Search history" autocomplete="off">
  <button class="pill danger" data-action="history-clear">Clear History…</button>
</div>
<div class="list" id="list">{rows}</div>
</div>"#
    );
    shell(
        PageKind::History,
        "History",
        &body,
        "",
        r#"
const filter = document.getElementById('filter');
filter.addEventListener('input', function () {
  const words = filter.value.toLowerCase().split(/\s+/).filter(Boolean);
  document.querySelectorAll('.history-row').forEach(function (row) {
    const hay = row.dataset.search || '';
    row.style.display = words.every(function (word) { return hay.indexOf(word) >= 0; }) ? '' : 'none';
  });
});
filter.focus();
"#,
        &prefs.look,
    )
}

// ----------------------------------------------------------------- bookmarks

fn bookmark_rows(nodes: &[crate::store::Bookmark]) -> String {
    let mut out = String::new();
    for node in nodes {
        if node.is_folder() {
            let children = node.children.as_deref().unwrap_or(&[]);
            out.push_str(&format!(
                r#"<div class="folder"><div class="folder-title">▾ {label}</div>{children}</div>"#,
                label = html(&node.label()),
                children = bookmark_rows(children)
            ));
        } else {
            let url = node.url.clone().unwrap_or_default();
            out.push_str(&format!(
                r#"<div class="row bookmark-row" data-search="{search}">
  <div class="icon">{letter}</div>
  <div class="grow"><div class="title">{title}</div><div class="detail">{url_display}</div></div>
  <div class="actions">
    <button class="pill plain" data-action="open" data-url="{url_attr}">Open</button>
    <button class="pill plain" data-action="bookmark-rename" data-id="{id}">Rename</button>
    <button class="pill plain" data-action="bookmark-delete" data-id="{id}">Remove</button>
  </div>
</div>"#,
                search = html(&format!("{} {}", node.title, url).to_lowercase()),
                letter = html(
                    &node
                        .title
                        .chars()
                        .next()
                        .or_else(|| store::bare_host(&url).chars().next())
                        .unwrap_or('•')
                        .to_string()
                ),
                title = html(&node.label()),
                url_display = html(&store::pretty_url(&url)),
                url_attr = html(&url),
                id = html(&node.id),
            ));
        }
    }
    let _ = bookmarks;
    out
}

pub fn bookmarks(bookmarks: &Bookmarks, prefs: &Prefs) -> String {
    let list = if bookmarks.empty() {
        "<div class=\"empty\">Nothing kept yet. Ctrl+Shift+B keeps the page you are on.</div>".into()
    } else {
        bookmark_rows(&bookmarks.roots)
    };
    let body = format!(
        r#"<div class="shell narrow">
<h1>Bookmarks</h1>
<p class="sub">The pages you keep. Folders are yours to arrange; a bookmark without a name shows only its mark on the bar.</p>
<div class="toolbar">
  <input class="search" id="filter" type="search" placeholder="Search bookmarks" autocomplete="off">
  <button class="pill" data-action="bookmark-folder">New Folder…</button>
  <button class="pill accent" data-action="bookmark-here">Keep This Page</button>
</div>
<div class="list" id="list">{list}</div>
</div>"#
    );
    shell(
        PageKind::Bookmarks,
        "Bookmarks",
        &body,
        "",
        r#"
const filter = document.getElementById('filter');
filter.addEventListener('input', function () {
  const words = filter.value.toLowerCase().split(/\s+/).filter(Boolean);
  document.querySelectorAll('.bookmark-row').forEach(function (row) {
    const hay = row.dataset.search || '';
    row.style.display = words.every(function (word) { return hay.indexOf(word) >= 0; }) ? '' : 'none';
  });
});
"#,
        &prefs.look,
    )
}

// ----------------------------------------------------------------- downloads

pub fn downloads(downloads: &[DownloadView], prefs: &Prefs) -> String {
    let body = format!(
        r#"<div class="shell narrow">
<h1>Downloads</h1>
<p class="sub">Files saved by Search, in {folder}.</p>
<div class="toolbar"><div class="grow"></div><button class="pill plain" data-action="downloads-clear">Clear Finished</button></div>
<div class="list" id="downloads">{list}</div>
</div>"#,
        folder = html(&prefs.downloads_path().to_string_lossy()),
        list = download_rows(downloads)
    );
    shell(
        PageKind::Downloads,
        "Downloads",
        &body,
        "",
        &format!(
            "window.__updateDownloads = function (list) {{ document.getElementById('downloads').innerHTML = list; }};\nconst initial = {};",
            serde_json::to_string(downloads).unwrap_or_else(|_| "[]".into())
        ),
        &prefs.look,
    )
}

pub fn download_rows(downloads: &[DownloadView]) -> String {
    if downloads.is_empty() {
        return "<div class=\"empty\">Nothing downloaded yet.</div>".into();
    }
    downloads
        .iter()
        .map(|download| {
            let percent = if download.total > 0 {
                (download.received as f64 / download.total as f64 * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            };
            let detail = if download.state == "Downloading" && download.total > 0 {
                format!(
                    "{} of {} · {}",
                    store::format_bytes(download.received),
                    store::format_bytes(download.total),
                    html(&download.host)
                )
            } else if download.state == "Downloading" || download.state == "Finished" {
                format!("{} · {}", store::format_bytes(download.received), html(&download.host))
            } else {
                html(&download.host)
            };
            let mut actions = String::new();
            if download.state == "Finished" {
                actions.push_str(&format!(
                    "<button class=\"pill plain\" data-action=\"download-open\" data-path=\"{}\">Open</button>",
                    html(&download.path)
                ));
                actions.push_str(&format!(
                    "<button class=\"pill plain\" data-action=\"download-reveal\" data-path=\"{}\">Show in Files</button>",
                    html(&download.path)
                ));
            } else if download.state == "Downloading" {
                actions.push_str(&format!(
                    "<button class=\"pill plain\" data-action=\"download-cancel\" data-index=\"{}\">Cancel</button>",
                    download.index
                ));
            }
            format!(
                r#"<div class="row">
  <div class="icon">↓</div>
  <div class="grow"><div class="title">{name}</div><div class="detail">{detail}</div>
  {progress}
  </div>
  <div class="actions">{actions}</div>
</div>"#,
                name = html(&download.name),
                detail = detail,
                progress = if download.state == "Downloading" && download.total > 0 {
                    format!(
                        "<div class=\"progress\"><i style=\"width:{:.1}%\"></i></div>",
                        percent
                    )
                } else {
                    String::new()
                },
                actions = actions
            )
        })
        .collect::<String>()
}

// ----------------------------------------------------------------- passwords

pub fn passwords(vault: &Vault, prefs: &Prefs) -> String {
    let entries = vault.all_sorted();
    let rows = if entries.is_empty() {
        "<div class=\"empty\">No passwords kept. Search offers to save one after you sign in somewhere.</div>".into()
    } else {
        entries
            .iter()
            .map(|login: &Login| {
                format!(
                    r#"<div class="row">
  <div class="icon">{letter}</div>
  <div class="grow"><div class="title">{user}</div><div class="detail">{host}</div></div>
  <div class="actions">
    <span class="pw-reveal" data-pw="{host_attr}|{user_attr}">••••••••</span>
    <button class="pill plain" data-action="password-show" data-host="{host_attr}" data-user="{user_attr}">Show</button>
    <button class="pill plain" data-action="password-fill" data-host="{host_attr}" data-user="{user_attr}">Fill Page</button>
    <button class="pill plain" data-action="password-delete" data-host="{host_attr}" data-user="{user_attr}">Remove</button>
  </div>
</div>"#,
                    letter = html(&login.user.chars().next().unwrap_or('•').to_string()),
                    user = html(if login.user.is_empty() { "(no account name)" } else { &login.user }),
                    host = html(&login.host),
                    host_attr = html(&login.host),
                    user_attr = html(&login.user)
                )
            })
            .collect::<String>()
    };
    let trouble = vault
        .trouble
        .as_ref()
        .map(|t| format!("<p class=\"sub\" style=\"color:var(--danger)\">{}</p>", html(t)))
        .unwrap_or_default();
    let body = format!(
        r#"<div class="shell narrow">
<h1>Passwords</h1>
{trouble}
<p class="sub">{count} sign-in{plural}, encrypted with AES-256-GCM. The key is a file only your user can read. Nothing is ever filled in until you ask.</p>
<div class="toolbar"><div class="grow"></div><button class="pill plain" data-action="go" data-page="settings-password">Password Settings…</button></div>
<div class="list">{rows}</div>
<div class="footer">Kept in <span class="mono">~/.local/share/Search/passwords.enc</span>. A local key, not a system keychain: on Linux this is the equivalent, and it is protected by your home directory's permissions.</div>
</div>"#,
        trouble = trouble,
        count = vault.count(),
        plural = if vault.count() == 1 { "" } else { "s" },
        rows = rows
    );
    shell(
        PageKind::Passwords,
        "Passwords",
        &body,
        "",
        r#"
window.__showPassword = function (host, user, password) {
  document.querySelectorAll('.pw-reveal').forEach(function (el) {
    if (el.dataset.pw === host + '|' + user) el.textContent = password;
  });
};
"#,
        &prefs.look,
    )
}

// -------------------------------------------------------------------- hidden

pub fn hidden(curtain: &Curtain, host: &str, prefs: &Prefs) -> String {
    let veils = curtain.veils(host);
    let rows = if veils.is_empty() {
        "<div class=\"empty\">Nothing hidden on this site. Press Ctrl+Shift+H, then click anything you would rather not see.</div>".into()
    } else {
        veils
            .iter()
            .map(|veil| {
                format!(
                    r#"<div class="row">
  <div class="icon">⌫</div>
  <div class="grow"><div class="title">{label}</div><div class="detail mono">{selector}{note}</div></div>
  <div class="actions"><button class="pill plain" data-action="veil-restore" data-host="{host_attr}" data-value="{selector_attr}">Put Back</button></div>
</div>"#,
                    label = html(if veil.label.is_empty() { &veil.selector } else { &veil.label }),
                    selector = html(&veil.selector),
                    note = if veil.note.is_empty() {
                        String::new()
                    } else {
                        format!(" · {}", html(&veil.note))
                    },
                    host_attr = html(host),
                    selector_attr = html(&veil.selector)
                )
            })
            .collect::<String>()
    };
    let other = curtain
        .all()
        .iter()
        .filter(|(h, _)| h.as_str() != host)
        .count();
    let body = format!(
        r#"<div class="shell narrow">
<h1>Hidden on This Site</h1>
<p class="sub">Everything taken off <b>{host_display}</b>, put back before the page draws next time. Nothing is hidden on any other site by these rules.</p>
<div class="toolbar"><div class="grow"></div>{undo}<button class="pill danger" data-action="veil-restore-all" data-host="{host_attr}">Put Everything Back</button></div>
<div class="list">{rows}</div>
{other}
<div class="footer">Hide something with Ctrl+Shift+H. Undo the last one while still pointing with Ctrl+Z.</div>
</div>"#,
        host_display = html(if host.is_empty() { "this page" } else { host }),
        host_attr = html(host),
        undo = if veils.is_empty() {
            String::new()
        } else {
            format!("<button class=\"pill plain\" data-action=\"veil-undo\" data-host=\"{}\">Undo Last</button>", html(host))
        },
        rows = rows,
        other = if other > 0 {
            format!("<p class=\"sub\">Rules are kept for {} other site{} too. Open Settings › Privacy to review them.</p>", other, if other == 1 { "" } else { "s" })
        } else {
            String::new()
        }
    );
    shell(PageKind::Hidden, "Hidden on This Site", &body, "", "", &prefs.look)
}

// ------------------------------------------------------------------- welcome

pub fn welcome(prefs: &Prefs) -> String {
    let engines = store::ENGINES
        .iter()
        .filter(|e| **e != "custom")
        .map(|e| {
            format!(
                "<option value=\"{}\"{}>{}</option>",
                e,
                if prefs.engine == *e { " selected" } else { "" },
                html(store::engine_title(e))
            )
        })
        .collect::<String>();
    let body = format!(
        r#"<div class="welcome"><div class="card">
{logo}
<h1>Search</h1>
<p>A small, quiet browser. One field takes an address or a few words; tabs stay out of the way. No account, no suggestions sent anywhere, no telemetry.</p>
<div class="set-row" style="border-bottom:0;margin-top:28px;padding-left:0;padding-right:0">
  <div class="label"><b>Search with</b><span>You can change this later in Settings.</span></div>
  <div class="control"><select data-setting="engine">{engines}</select></div>
</div>
<div style="display:flex;gap:8px;margin-top:26px">
  <button class="pill accent" data-action="welcome-done">Start Browsing</button>
  <button class="pill" data-action="go" data-page="settings">Settings…</button>
</div>
<div class="footer">Take your time. Ctrl+L goes to the address field from anywhere.</div>
</div></div>"#,
        logo = logo_svg(),
        engines = engines
    );
    shell(PageKind::Welcome, "Welcome", &body, "", "", &prefs.look)
}

// --------------------------------------------------------------------- blank

pub fn blank(prefs: &Prefs) -> String {
    let body = r#"<div style="height:100vh"></div>"#.to_string();
    shell(
        PageKind::Blank,
        "New Tab",
        &body,
        "body{background:var(--ground)}",
        "",
        &prefs.look,
    )
}

// ---------------------------------------------------------------------- help

pub fn post_json(value: serde_json::Value) -> String {
    format!("JSON.stringify({})", value)
}

pub fn downloads_json(downloads: &[DownloadView]) -> String {
    serde_json::to_string(downloads).unwrap_or_else(|_| "[]".into())
}

pub fn logins_json(logins: &[Login]) -> serde_json::Value {
    json!(logins
        .iter()
        .map(|l| json!({"host": l.host, "user": l.user}))
        .collect::<Vec<_>>())
}

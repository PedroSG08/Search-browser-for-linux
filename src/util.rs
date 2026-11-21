// Small shared pieces: GTK widgets that look like Search's, and the stylesheet
// for the frame. Every colour is a pair and the window picks one.

use gtk::prelude::*;

pub fn html(text: &str) -> String {
    crate::store::html_escape(text)
}

pub fn label(text: &str, class: Option<&str>) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    if let Some(class) = class {
        label.style_context().add_class(class);
    }
    label
}

pub fn image_icon(name: &str, size: i32) -> gtk::Image {
    let image = gtk::Image::from_icon_name(Some(name), gtk::IconSize::Menu);
    image.set_pixel_size(size);
    image
}

pub fn button_icon(name: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::new();
    button.add(&image_icon(name, 15));
    button.set_relief(gtk::ReliefStyle::None);
    button.set_tooltip_text(Some(tooltip));
    button.style_context().add_class("icon-button");
    button
}

pub fn button_text(text: &str) -> gtk::Button {
    let button = gtk::Button::with_label(text);
    button.set_relief(gtk::ReliefStyle::None);
    button.style_context().add_class("plain-button");
    button
}

/// Show a widget and every descendant, ignoring `no_show_all`. `show_all()`
/// does not descend into a widget that itself has `no_show_all` set, so the
/// omnibox card, find bar and toast — hidden until first use — need this.
pub fn reveal<W: IsA<gtk::Widget>>(widget: &W) {
    widget.show();
    if let Some(container) = widget.dynamic_cast_ref::<gtk::Container>() {
        container.foreach(reveal);
    }
}

pub fn hbox(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Horizontal, spacing)
}

pub fn vbox(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, spacing)
}

pub fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    let scroll = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.add(child);
    scroll
}

pub fn separator() -> gtk::Separator {
    gtk::Separator::new(gtk::Orientation::Horizontal)
}

pub fn trim(text: &str) -> &str {
    text.trim()
}

// ----------------------------------------------------------------- css

const CSS: &str = r#"
* { outline: none; }
window {
    background-color: @ground; color: @ink;
    font-family: "Noto Sans", "Cantarell", "DejaVu Sans", sans-serif;
    font-size: 12.5px;
}
.window-ground, .page-ground { background-color: @ground; color: @ink; }
.window-ground.carded { background-color: @frame; }

/* ---- the window's own title bar: the lights and the strip */
.chrome { background-color: @ground; min-height: 52px; border: none; }
.side-top { background-color: transparent; min-height: 52px; }
button.traffic {
    background-image: none; border: none; box-shadow: none; padding: 0;
    min-width: 12px; min-height: 12px; border-radius: 6px;
}
button.traffic-close { background-color: #ff5f57; }
button.traffic-min { background-color: #febc2e; }
button.traffic-max { background-color: #28c840; }
button.traffic:hover { opacity: 0.85; }
.helm-button {
    background-image: none; background-color: transparent; border: none; box-shadow: none;
    color: @muted; padding: 0; min-width: 26px; min-height: 26px; border-radius: 7px;
}
.helm-button:hover { background-color: @hover; color: @ink; }
.helm-button:disabled { opacity: 0.3; }
.strip-door {
    background-image: none; background-color: transparent; border: none; box-shadow: none;
    color: @muted; padding: 0; min-width: 26px; min-height: 26px; border-radius: 7px;
}
.strip-door:hover { background-color: @hover; color: @ink; }
.strip-door > button, .foot-button > button {
    background-image: none; background-color: transparent; border: none; box-shadow: none;
    padding: 0; min-width: 24px; min-height: 24px; color: @muted;
}
.tab-strip { background-color: transparent; border: none; }
.tab-pill {
    background-image: none; background-color: transparent; border: none; box-shadow: none;
    border-radius: 9px; padding: 0; min-height: 28px;
}
.tab-pill.pinned, .tab-pill.compact { background-color: alpha(@wash, 0.55); }
.tab-pill:hover { background-color: @hover; }
.tab-pill.active, .tab-pill.compact.active { background-color: @wash; }
.tab-pill.pinned.active { background-color: @pinLive; }
.tab-pill.pinned { border-radius: 9px; }
.pin-line { background-color: @hairline; }
.tab-plus {
    background-image: none; background-color: transparent; border: none; box-shadow: none;
    color: @muted; padding: 0; min-width: 30px; min-height: 28px; border-radius: 9px;
}
.tab-plus:hover { background-color: @hover; color: @ink; }

/* ---- the column */
.sidebar { background-color: @ground; border-right: 1px solid @hairline; }
.sidebar.right { border-right: none; border-left: 1px solid @hairline; }
.sidebar.on-frame { background-color: @frame; border-color: transparent; }
.tab-row {
    background-image: none; background-color: transparent; border: none; box-shadow: none;
    border-radius: 9px; padding: 0; min-height: 28px; color: @muted;
}
.tab-row:hover { background-color: @hover; }
.tab-row.active { background-color: @wash; color: @ink; }
.tab-row.roomy { min-height: 36px; }
.tab-title { font-size: 12.5px; color: @muted; }
.tab-row.active .tab-title, .tab-pill.active .tab-title, .tab-row:hover .tab-title,
.tab-pill:hover .tab-title { color: @ink; }
.tab-row.roomy .tab-title { font-size: 14px; }
.tab-badge { font-size: 11px; font-weight: 600; color: @muted; min-width: 15px; }
.tab-row.active .tab-badge, .tab-pill.active .tab-badge { color: @ink; }
.tab-speaker { color: @muted; }
.tab-close {
    padding: 0; min-width: 15px; min-height: 15px; border-radius: 8px;
    background-image: none; background-color: transparent; border: none; box-shadow: none;
    color: @muted;
}
.tab-close:hover { background-color: alpha(@ink, 0.10); color: @ink; }
.tab-close-glyph { font-size: 11px; font-weight: 700; color: @muted; }

.pin {
    background-image: none; background-color: @hover; border: none; box-shadow: none;
    border-radius: 9px; padding: 0; min-width: 20px; min-height: 34px; color: @muted;
}
.pin:hover { background-color: @wash; color: @ink; }
.pin.active { background-color: @pinLive; color: @ink; }
.pin-letter { font-size: 12px; font-weight: 500; }
.new-tab-row {
    background-image: none; background-color: transparent; border: none; box-shadow: none;
    border-radius: 9px; padding: 0; min-height: 28px;
}
.new-tab-row:hover { background-color: @hover; }
.new-tab-label { font-size: 12.5px; color: @muted; }
.new-tab-row:hover .new-tab-label { color: @ink; }
.sidebar-foot { background-color: transparent; border: none; }
.foot-button {
    background-image: none; background-color: transparent; border: none; box-shadow: none;
    color: @muted; padding: 0; min-width: 26px; min-height: 26px; border-radius: 7px;
}
.foot-button:hover { background-color: @hover; color: @ink; }
.sidebar.on-frame .tab-row:hover { background-color: @sideHover; }
.sidebar.on-frame .tab-row.active { background-color: @sideWash; }
.sidebar.on-frame .pin { background-color: @sideTile; }
.sidebar.on-frame .pin:hover { background-color: @sideHover; }
.sidebar.on-frame .pin.active { background-color: @sidePinLive; }
.sidebar.on-frame .new-tab-row:hover { background-color: @sideHover; }
.sidebar-edge { background-color: transparent; border: none; box-shadow: none; }

/* ---- the title bar that slides down from the top edge */
.top-edge { background-color: transparent; border: none; box-shadow: none; }
.top-bar {
    background-color: @ground; border-bottom: 1px solid @hairline;
    border-radius: 0; box-shadow: none; min-height: 46px;
}
.top-bar.carded { background-color: @frame; border-bottom: none; border-bottom-left-radius: 10px; border-bottom-right-radius: 10px; }
.top-address {
    background-image: none; background-color: alpha(@wash, 0.55); border: none;
    box-shadow: none; border-radius: 8px; padding: 0 10px; min-height: 26px;
}
.top-address:hover { background-color: @wash; }
.top-address label { color: @muted; font-size: 12px; }
.top-address:hover label { color: @ink; }
.url-on-frame { background-color: @frame; }
.url-on-ground { background-color: @ground; }
.url-sidebar-drop { border-bottom-left-radius: 10px; border-bottom-right-radius: 10px; }

/* ---- the page */
.page-card {
    background-color: @ground; border-radius: 10px;
    border: 1px solid alpha(@ink, 0.08);
    box-shadow: 0 1px 6px alpha(black, 0.07);
}
.blank-page { background-color: @ground; }

/* ---- menus and the floating address field */
.menu-popover { background-color: @ground; }
menu, .menu-popover contents {
    background-color: @ground; border: 1px solid @hairline; border-radius: 10px;
    box-shadow: 0 10px 30px alpha(black, 0.14); padding: 5px;
}
.menu-row {
    background-image: none; background-color: transparent; border: none; box-shadow: none;
    border-radius: 7px; padding: 5px 9px; min-height: 24px;
}
.menu-row:hover { background-color: @wash; }
.menu-label { font-size: 12.5px; color: @ink; }
.menu-accel { font-size: 11px; color: @muted; }

.omnibox-dim { background-color: alpha(@ground, 0.72); }
.omnibox-card {
    background-color: @ground; border: 1px solid @hairline; border-radius: 13px;
    box-shadow: 0 10px 30px alpha(black, 0.13); padding: 8px;
}
.omnibox-entry {
    background-image: none; background-color: transparent; border: none; box-shadow: none;
    font-size: 16px; color: @ink; caret-color: @ink; padding: 6px 9px; min-height: 28px;
}
.omnibox-list {
    background-color: @ground; border: 1px solid @hairline; border-radius: 11px; padding: 5px;
    box-shadow: 0 8px 24px alpha(black, 0.10);
}
.omnibox-row {
    border-radius: 8px; padding: 7px 10px; min-height: 26px;
    background-image: none; background-color: transparent; border: none; box-shadow: none;
}
.omnibox-row:hover, .omnibox-row:selected, .omnibox-row.picked { background-color: @wash; }
.suggestion-title { font-size: 12.5px; color: @ink; }
.suggestion-kind { font-size: 10px; color: @muted; }
.suggestion-icon { color: @muted; }
.address-button { background: transparent; border: none; box-shadow: none; }

.welcome-mark { color: @ink; }
.welcome-title { font-size: 34px; font-weight: 600; color: @ink; }
.welcome-sub { font-size: 14px; color: @muted; }

.toast {
    background-color: @ground; border: 1px solid @hairline; border-radius: 18px;
    box-shadow: 0 6px 18px alpha(black, 0.10); color: @ink; padding: 8px 15px; font-size: 12px;
}
.toast .action { color: @ink; font-weight: 600; }
.link-status {
    background-color: @ground; border: 1px solid @hairline; border-radius: 7px;
    color: @muted; font-size: 11px; padding: 4px 8px;
}
.find-card {
    background-color: @ground; border: 1px solid @hairline; border-radius: 10px;
    box-shadow: 0 6px 18px alpha(black, 0.10); padding: 6px;
}
.find-entry {
    background-image: none; background-color: transparent; border: none; box-shadow: none;
    min-height: 24px; font-size: 13px; color: @ink; padding: 2px 6px;
}
.find-status { font-size: 11px; color: @muted; padding: 0 6px; }
.find-button {
    background-image: none; background-color: transparent; border: none; box-shadow: none;
    color: @muted; border-radius: 6px; padding: 2px 4px; min-width: 20px; min-height: 20px;
}
.find-button:hover { background-color: @hover; color: @ink; }

.bookmarks-bar {
    background-color: @ground; border-bottom: 1px solid @hairline; padding: 3px 10px;
    min-height: 30px;
}
.bookmarks-bar button {
    background-image: none; background-color: transparent; border: none; box-shadow: none;
    color: @muted; border-radius: 6px; padding: 3px 7px; font-size: 12px;
}
.bookmarks-bar button:hover { background-color: @hover; color: @ink; }

.list-row { padding: 9px 10px; border-radius: 8px; }
.list-row:hover { background-color: @hover; }
.list-title { font-size: 13px; color: @ink; }
.list-sub { font-size: 11.5px; color: @muted; }
.empty-state { color: @muted; font-size: 13px; padding: 30px; }
.danger { color: #c0392b; }
.accent-button {
    background-image: none; background-color: @ink; color: @ground; border: none;
    border-radius: 16px; padding: 8px 18px; font-weight: 500;
}
.accent-button:hover { background-color: alpha(@ink, 0.85); }
.plain-button {
    background-image: none; background-color: transparent; border: 1px solid @hairline;
    border-radius: 8px; padding: 6px 12px; color: @ink;
}
.plain-button:hover { background-color: @hover; }

scrollbar { background-color: transparent; border: none; }
scrollbar slider {
    background-color: alpha(@muted, 0.30); border-radius: 4px; min-width: 5px; min-height: 5px;
    border: 2px solid transparent; background-clip: content-box;
}
scrollbar slider:hover { background-color: alpha(@muted, 0.50); }
menuitem { padding: 5px 10px; }
tooltip { background-color: @ink; color: @ground; border-radius: 6px; }
"#;

/// The window ground under the page card, in the corner masks' colour.
pub fn frame_rgba(look: &str) -> gdk::RGBA {
    let dark = match look {
        "dark" => true,
        "light" => false,
        _ => system_prefers_dark(),
    };
    if dark {
        gdk::RGBA::new(19.0 / 255.0, 19.0 / 255.0, 19.0 / 255.0, 1.0)
    } else {
        gdk::RGBA::new(244.0 / 255.0, 244.0 / 255.0, 243.0 / 255.0, 1.0)
    }
}

/// The card's 1 px border, already blended over its ground colour. The corner
/// patches use it so the arc continues the straight border exactly.
pub fn card_edge_rgba(look: &str) -> gdk::RGBA {
    let dark = match look {
        "dark" => true,
        "light" => false,
        _ => system_prefers_dark(),
    };
    if dark {
        gdk::RGBA::new(45.0 / 255.0, 45.0 / 255.0, 45.0 / 255.0, 1.0)
    } else {
        gdk::RGBA::new(236.0 / 255.0, 236.0 / 255.0, 236.0 / 255.0, 1.0)
    }
}

pub fn css_for(look: &str) -> String {
    let dark = match look {
        "dark" => true,
        "light" => false,
        _ => system_prefers_dark(),
    };
    let (ground, ink, muted, faint, hairline, wash, frame, hover, pin_live) = if dark {
        ("#1c1c1c", "#ededed", "#949494", "#525252", "#333333", "#2d2d2d", "#131313", "#262626", "#363636")
    } else {
        ("#ffffff", "#171717", "#8c8c8c", "#d4d4d4", "#e8e8e8", "#efefef", "#f4f4f3", "#f6f6f6", "#e6e6e6")
    };
    let (side_wash, side_hover, side_tile, side_pin_live) = if dark {
        ("#303030", "#252525", "#212121", "#3a3a3a")
    } else {
        ("#e3e3e5", "#ededed", "#ebebeb", "#d9d9dc")
    };
    format!(
        "@define-color ground {};\n@define-color ink {};\n@define-color muted {};\n\
         @define-color faint {};\n@define-color hairline {};\n@define-color wash {};\n\
         @define-color frame {};\n@define-color hover {};\n@define-color pinLive {};\n\
         @define-color sideWash {};\n@define-color sideHover {};\n@define-color sideTile {};\n\
         @define-color sidePinLive {};\n{}",
        ground, ink, muted, faint, hairline, wash, frame, hover, pin_live, side_wash, side_hover,
        side_tile, side_pin_live, CSS
    )
}

/// Whether the desktop asks for a dark interface. GTK3 has no single API for
/// this: themes name themselves `*-dark`, and GNOME exposes `color-scheme`.
/// Deliberately ignores `gtk-application-prefer-dark-theme`, because this app
/// sets that property itself when the user picks Light or Dark.
pub fn system_prefers_dark() -> bool {
    if let Some(settings) = gtk::Settings::default() {
        let theme = settings
            .property::<Option<String>>("gtk-theme-name")
            .unwrap_or_default();
        if theme.to_lowercase().contains("dark") {
            return true;
        }
    }
    if let Some(source) = gio::SettingsSchemaSource::default() {
        if source.lookup("org.gnome.desktop.interface", true).is_some() {
            let settings = gio::Settings::new("org.gnome.desktop.interface");
            if settings.string("color-scheme") == "prefer-dark" {
                return true;
            }
        }
    }
    false
}

pub fn install_css(look: &str) {
    let provider = gtk::CssProvider::new();
    provider
        .load_from_data(css_for(look).as_bytes())
        .ok();
    if let Some(screen) = gdk::Screen::default() {
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_USER,
        );
    }
}

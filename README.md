# Search (Linux)

Native Linux port of the macOS **Search** browser,
rebuilt in Rust on top of GTK 3.24 and WebKitGTK 4.1. The original macOS
sources are left untouched; every piece that could not be recompiled was
rewritten in Rust.

## Install from GitHub

```
git clone https://github.com/PedroSG08/Search-browser-for-linux.git
cd Search-browser-for-linux
```

Dependencies (Debian/Ubuntu):

```
sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev libgdk-pixbuf-2.0-dev \
    libglib2.0-dev libcairo2-dev libpango1.0-dev
```

Dependencies (Arch Linux):

```
sudo pacman -S gtk3 webkit2gtk-4.1 gdk-pixbuf2 glib2 cairo pango
```

Build:

```
cargo build --release
```

The binary is `target/release/search`.

## Run

```
./target/release/search [url...]
```

Internal pages are real `search://` URLs:

- `search://settings`
- `search://history`
- `search://bookmarks`
- `search://downloads`
- `search://passwords`
- `search://hidden`
- `search://welcome`
- `search://blank`

Settings are stored in `~/.config/Search/settings.json`.
Data (history, bookmarks, session, downloads, favicons, WebKit storage) lives in
`~/.local/share/Search`. Cookies and site storage persist in
`~/.local/share/Search/web`, so sign-ins survive a normal quit and relaunch.

Tabs restored from the last session load only when you first open them, inactive
tabs are put to sleep after ten minutes (Settings → Tabs), and closing a tab
releases its web process. This keeps memory low when many tabs are open; a
sleeping tab simply reloads when you return to it.

With tabs in the side column, the top edge reveals a slim title bar with the
traffic lights, back/forward/reload and the current address. It slides away
again when the pointer returns to the page, even over PDFs and embedded
content.

Passwords are kept in an encrypted vault at
`~/.local/share/Search/passwords.key` (AES-256-GCM key, mode 0600) and
`~/.local/share/Search/passwords.enc`, replacing the macOS Keychain.

## Keyboard

Ctrl replaces Command from the macOS build: Ctrl+T new tab, Ctrl+W close tab,
Ctrl+L focus address, Ctrl+F find, Ctrl+, settings, Ctrl+H history, Ctrl+J
downloads, Ctrl+Shift+B bookmarks, Ctrl+Alt+N split view, Ctrl+Shift+R reader,
Ctrl+Shift+H hide element, Ctrl+Shift+S toggle sidebar.

## Platform notes / substitutions

- `WKWebExtension` is not available in WebKitGTK, so Chrome (`chrome.*`) and
  Firefox (`browser.*`) extensions cannot be installed. Search uses WebKit's
  native content blockers instead, compiled from uBlock Origin's official
  filter lists (uBlock filters, Badware, Privacy, Quick fixes, Unbreak,
  EasyList, EasyPrivacy, Peter Lowe), enabled by default.
- Apple Passwords / passkeys / AppleScript / IOKit have no direct Linux
  equivalents; the encrypted local vault substitutes for Keychain, and other
  macOS-only integrations are documented as unavailable.
- The built-in AI assistant, Sparkle self-updater, and Safari-style reader
  heuristics are not ported; reader mode keeps its own simpler implementation.
- The macOS icon was redrawn as `data/search.svg` (white plate, subtract mark).
- The application id is `com.officecommun.Search`.

## Known limitations

- Wayland/Hyprland is the primary target; X11 and Broadway also work, but
  WebKit content layout under Broadway needs `gtk-xft-dpi` normalisation
  (applied automatically).
- External `https` sites may fail on networks where TLS verification is
  unavailable; this is an environment limitation, not an app bug.
- uBlock Origin rules that replace responses, redirect requests, strip query
  parameters or use procedural cosmetic filters have no WebKit content-blocker
  equivalent and are skipped.

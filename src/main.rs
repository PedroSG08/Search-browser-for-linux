#![allow(dead_code, deprecated)]
// Search for Linux. One window, drawn with GTK, showing pages with WebKitGTK.

mod browser;
mod pages;
mod ublock;
mod store;
mod util;

use std::cell::RefCell;
use std::rc::Rc;

use gio::prelude::*;
use gtk::prelude::*;

thread_local! {
    // The window is held here because every signal inside it speaks to the
    // browser through a weak reference. Strong on the outside, weak within.
    static WINDOW: RefCell<Option<Rc<browser::Browser>>> = const { RefCell::new(None) };
}

fn main() -> glib::ExitCode {
    // WebKitGTK renders faster here with the cairo path. Compositing stays on:
    // disabling it made the web process crash during YouTube ad playback.
    std::env::set_var("GSK_RENDERER", "cairo");
    glib::set_application_name("Search");
    if std::env::args().any(|argument| argument == "--ublock-stats") {
        println!("{}", ublock::summary());
        return glib::ExitCode::SUCCESS;
    }
    if let Some(path) = std::env::args().find_map(|argument| {
        argument
            .strip_prefix("--ublock-json=")
            .map(|path| path.to_string())
    }) {
        let json = ublock::rules_json();
        if std::fs::write(&path, json).is_err() {
            eprintln!("Search: could not write {}", path);
            return glib::ExitCode::FAILURE;
        }
        return glib::ExitCode::SUCCESS;
    }
    let app = gtk::Application::new(
        Some("com.officecommun.Search"),
        gio::ApplicationFlags::empty(),
    );

    let urls: Vec<String> = std::env::args()
        .skip(1)
        .filter(|argument| !argument.starts_with('-'))
        .collect();

    app.connect_activate(move |app| {
        let existing = WINDOW.with(|slot| slot.borrow().clone());
        if let Some(browser) = existing {
            browser.window.present();
            if let Some(view) = browser.active_view() {
                view.grab_focus();
            }
            return;
        }

        let browser = browser::Browser::new(app);
        browser.start(urls.clone());
        browser.window.present();

        let weak = Rc::downgrade(&browser);
        browser.window.connect_destroy(move |_| {
            WINDOW.with(|slot| {
                let same = slot
                    .borrow()
                    .as_ref()
                    .map(|held| weak.ptr_eq(&Rc::downgrade(held)))
                    .unwrap_or(false);
                if same {
                    *slot.borrow_mut() = None;
                }
            });
        });
        WINDOW.with(|slot| *slot.borrow_mut() = Some(browser));
    });

    let program = std::env::args()
        .next()
        .unwrap_or_else(|| "search".to_string());
    app.run_with_args(&[program])
}

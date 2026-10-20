#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod deepl;
mod i18n;
mod languages;
mod messages;
mod placement;
mod settings;
mod state;
mod windows_integration;

#[cfg(windows)]
use slint::winit_030::winit::event::WindowEvent;
#[cfg(windows)]
use slint::winit_030::winit::platform::windows::WindowAttributesExtWindows;
#[cfg(windows)]
use slint::winit_030::{EventResult, WinitWindowAccessor};
use slint::{CloseRequestResponse, ComponentHandle, Timer};

use app::App;
use settings::Preferences;
use state::AppState;

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Skia draws on the CPU here. Since Slint 1.18 its GPU path on Windows goes
    // through wgpu, which holds about 90 MB more for this small popup.
    let backend = slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("skia-software".into());
    #[cfg(windows)]
    let backend = backend.with_winit_window_attributes_hook(|attributes| {
        attributes.with_skip_taskbar(true).with_transparent(true)
    });
    backend.select()?;

    let popup = MainWindow::new()?;
    let settings_window = SettingsWindow::new()?;
    let tray = TrayIcon::new()?;
    let version = display_version();
    settings_window.set_version(version.clone());
    tray.set_version(version);

    // The language comes first so every message after this is in it.
    let preferences = Preferences::load();
    i18n::set(preferences.interface_language.resolve());

    let api_key_result = settings::load_api_key();
    let state = AppState::new(
        api_key_result
            .as_ref()
            .ok()
            .and_then(Clone::clone)
            .unwrap_or_default(),
        preferences,
        windows_integration::current_anchor(),
    );
    let app = App::new(&popup, &settings_window, &tray, state);
    app.apply_theme();
    app.apply_interface_language(i18n::current());
    if let Err(error) = api_key_result {
        settings_window.set_message(error.into());
    }

    connect_popup(&popup, &app);
    connect_settings(&settings_window, &app);
    connect_tray(&tray, &app);

    let hotkey_listener = {
        let app = app.clone();
        windows_integration::start_hotkey_listener(move |capture_result| {
            let fallback_anchor = windows_integration::current_anchor();
            let app = app.clone();
            let _ = slint::invoke_from_event_loop(move || {
                app.handle_capture(capture_result, fallback_anchor);
            });
        })
    };
    let hotkey_listener = match hotkey_listener {
        Ok(listener) => Some(listener),
        Err(error) => {
            app.show_error(
                &messages::Message::HotkeyUnavailableTitle.text(),
                &error,
                false,
            );
            app.show_popup(windows_integration::current_anchor());
            None
        }
    };

    {
        let app = app.clone();
        windows_integration::watch_theme_changes(move || {
            let app = app.clone();
            let _ = slint::invoke_from_event_loop(move || app.apply_theme());
        });
    }

    tray.show()?;
    app.refresh_target_languages();
    slint::run_event_loop()?;
    drop(hotkey_listener);
    Ok(())
}

fn connect_popup(popup: &MainWindow, app: &App) {
    popup
        .window()
        .on_close_requested(|| CloseRequestResponse::HideWindow);
    #[cfg(windows)]
    {
        // Like the original, the popup closes as soon as it loses focus.
        let app = app.clone();
        popup.window().on_winit_window_event(move |_, event| {
            if let WindowEvent::Focused(false) = event {
                let app = app.clone();
                Timer::single_shot(app::LOST_FOCUS_CLOSE_DELAY, move || {
                    app.close_popup_if_inactive()
                });
            }
            EventResult::Propagate
        });
    }

    let popup_weak = popup.as_weak();
    popup.on_drag_requested(move || {
        if let Some(popup) = popup_weak.upgrade() {
            placement::drag_window(popup.window());
        }
    });
    let popup_weak = popup.as_weak();
    popup.on_resize_requested(move || {
        if let Some(popup) = popup_weak.upgrade() {
            placement::resize_window(popup.window());
        }
    });

    let handler = app.clone();
    popup.on_hide_requested(move || handler.hide_popup());
    let handler = app.clone();
    popup.on_settings_requested(move || {
        handler.hide_popup();
        handler.open_settings(windows_integration::current_anchor(), None);
    });
    let handler = app.clone();
    popup.on_menu_toggled(move || handler.fit_window_to_menu());
    let handler = app.clone();
    popup.on_language_selected(move |code| handler.select_language(&code));
    let handler = app.clone();
    popup.on_language_search_changed(move |_| handler.refresh_language_ui());
    let handler = app.clone();
    popup.on_search_accepted(move || handler.select_first_match());
    let handler = app.clone();
    popup.on_copy_requested(move || handler.copy_translation());
    let handler = app.clone();
    popup.on_replace_requested(move || handler.replace_selection());
    let handler = app.clone();
    popup.on_retry_requested(move || handler.begin_translation(false));
}

fn connect_settings(settings_window: &SettingsWindow, app: &App) {
    let handler = app.clone();
    settings_window.window().on_close_requested(move || {
        handler.hide_settings();
        CloseRequestResponse::KeepWindowShown
    });

    let settings_weak = settings_window.as_weak();
    settings_window.on_drag_requested(move || {
        if let Some(settings_window) = settings_weak.upgrade() {
            placement::drag_window(settings_window.window());
        }
    });
    let handler = app.clone();
    settings_window.on_save_settings(move |api_key, free_api, launch_at_startup, language| {
        handler.save_settings(
            api_key.trim(),
            free_api,
            launch_at_startup,
            i18n::InterfaceLanguage::from_index(language),
        );
    });
    let handler = app.clone();
    settings_window.on_cancel_settings(move || handler.hide_settings());
}

fn connect_tray(tray: &TrayIcon, app: &App) {
    tray.on_menu_requested(|| {
        if let Err(error) = windows_integration::show_tray_menu() {
            eprintln!("failed to show tray menu: {error}");
        }
    });
    let handler = app.clone();
    tray.on_open_requested(move || handler.open_from_tray());
    let handler = app.clone();
    tray.on_settings_requested(move || {
        handler.hide_popup();
        handler.open_settings(windows_integration::current_anchor(), None);
    });
    tray.on_quit_requested(|| {
        let _ = slint::quit_event_loop();
    });
}

/// The package version as shown to users: "v0.1" for 0.1.0, "v0.1.2" for 0.1.2.
fn display_version() -> slint::SharedString {
    let patch = env!("CARGO_PKG_VERSION_PATCH");
    let base = format!(
        "v{}.{}",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR")
    );
    if patch == "0" {
        base.into()
    } else {
        format!("{base}.{patch}").into()
    }
}

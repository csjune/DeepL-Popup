//! What the popup, the settings window and the tray do.

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use slint::{ComponentHandle, LogicalSize, ModelRc, Timer, TimerMode, VecModel};

use crate::deepl::{self, TranslateError, Translation};
use crate::languages::{self, TargetLanguage};
use crate::placement;
use crate::settings::{self, Preferences};
use crate::state::{self, AppState};
use crate::windows_integration::{self, PopupAnchor, SelectionCapture};
use crate::{MainWindow, SettingsWindow, TrayIcon};

pub const PLACEHOLDER_TEXT: &str = "텍스트를 선택한 다음 Ctrl+F9를 누르세요.";
const MISSING_KEY_MESSAGE: &str = "번역하려면 DeepL API 인증 키를 입력하세요.";
pub const LOST_FOCUS_CLOSE_DELAY: Duration = Duration::from_millis(50);
const OUTSIDE_CLICK_POLL_INTERVAL: Duration = Duration::from_millis(45);
/// Lets the hidden window finish its last frame before memory is released.
const TRIM_DELAY: Duration = Duration::from_millis(500);

thread_local! {
    // Runs only while the popup is open; see `sync_watchers`.
    static OUTSIDE_CLICK_TIMER: Timer = Timer::default();
}

/// Handle to the app that UI callbacks and request threads share.
#[derive(Clone)]
pub struct App {
    popup: slint::Weak<MainWindow>,
    settings: slint::Weak<SettingsWindow>,
    tray: slint::Weak<TrayIcon>,
    state: Arc<Mutex<AppState>>,
    request_serial: Arc<AtomicU64>,
}

impl App {
    pub fn new(
        popup: &MainWindow,
        settings: &SettingsWindow,
        tray: &TrayIcon,
        state: AppState,
    ) -> Self {
        Self {
            popup: popup.as_weak(),
            settings: settings.as_weak(),
            tray: tray.as_weak(),
            state: Arc::new(Mutex::new(state)),
            request_serial: Arc::new(AtomicU64::new(0)),
        }
    }

    fn state(&self) -> MutexGuard<'_, AppState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn handle_capture(
        &self,
        capture_result: Result<SelectionCapture, String>,
        fallback_anchor: PopupAnchor,
    ) {
        let Some(popup) = self.popup.upgrade() else {
            return;
        };
        match capture_result {
            Ok(capture) => {
                let api_key_missing = {
                    let mut state = self.state();
                    state.set_source(capture.text, capture.source_window);
                    state.api_key.trim().is_empty()
                };
                popup.set_result_text("".into());
                popup.set_has_result(false);
                popup.set_error_visible(false);
                if api_key_missing {
                    self.hide_popup();
                    self.open_settings(capture.anchor, Some(MISSING_KEY_MESSAGE));
                    return;
                }
                self.show_popup(capture.anchor);
                self.begin_translation(true);
            }
            Err(error) => {
                self.request_serial.fetch_add(1, Ordering::SeqCst);
                self.state().clear_source();
                self.show_error("텍스트를 가져올 수 없습니다", &error, false);
                self.show_popup(fallback_anchor);
            }
        }
    }

    /// Translates the current source text into the current target language.
    /// The first translation of a selection may switch the target when the text
    /// is already written in it, like the original does.
    pub fn begin_translation(&self, allow_target_switch: bool) {
        let Some(popup) = self.popup.upgrade() else {
            return;
        };
        let request_id = self.request_serial.fetch_add(1, Ordering::SeqCst) + 1;
        let (source_text, target, api_key, use_free_api, cached) = {
            let state = self.state();
            (
                state.source_text.clone(),
                state.target_language().clone(),
                state.api_key.clone(),
                state.use_free_api,
                state.translation().map(str::to_owned),
            )
        };

        if source_text.trim().is_empty() {
            popup.set_loading(false);
            popup.set_has_result(false);
            popup.set_error_visible(false);
            popup.set_result_text("".into());
            return;
        }
        if api_key.trim().is_empty() {
            self.show_error("DeepL API 인증 키가 필요합니다", MISSING_KEY_MESSAGE, true);
            return;
        }
        if let Some(translation) = cached {
            self.show_translation(&translation);
            return;
        }

        popup.set_error_visible(false);
        popup.set_has_result(false);
        popup.set_loading(true);

        let app = self.clone();
        thread::spawn(move || {
            let result = deepl::translate(&source_text, &target.code, &api_key, use_free_api);
            let _ = slint::invoke_from_event_loop(move || {
                app.finish_translation(request_id, &target, allow_target_switch, result);
            });
        });
    }

    fn finish_translation(
        &self,
        request_id: u64,
        target: &TargetLanguage,
        allow_target_switch: bool,
        result: Result<Translation, TranslateError>,
    ) {
        if self.request_serial.load(Ordering::SeqCst) != request_id {
            return;
        }
        match result {
            Ok(translation) => {
                if allow_target_switch
                    && let Some(source_language) = translation.detected_source_language.as_deref()
                    && languages::same_family(source_language, &target.code)
                    && self.switch_target_away_from(source_language, target)
                {
                    self.refresh_language_ui();
                    self.begin_translation(false);
                    return;
                }
                self.state()
                    .store_translation(&target.code, translation.text.clone());
                self.show_translation(&translation.text);
            }
            Err(error) => {
                let presentation = error.presentation();
                self.show_error(
                    presentation.title,
                    &presentation.description,
                    presentation.needs_settings,
                );
            }
        }
    }

    /// Makes the fallback for `source_language` the target. Returns whether the
    /// target changed.
    fn switch_target_away_from(&self, source_language: &str, target: &TargetLanguage) -> bool {
        let mut state = self.state();
        let fallback = state.fallback_target(source_language).code.clone();
        if fallback.eq_ignore_ascii_case(&target.code) {
            return false;
        }
        state.add_suggested(&fallback);
        state.save_preferences();
        true
    }

    fn show_translation(&self, text: &str) {
        let Some(popup) = self.popup.upgrade() else {
            return;
        };
        popup.set_loading(false);
        popup.set_error_visible(false);
        popup.set_result_text(text.into());
        popup.set_has_result(true);
    }

    pub fn show_error(&self, title: &str, message: &str, needs_settings: bool) {
        let Some(popup) = self.popup.upgrade() else {
            return;
        };
        popup.set_loading(false);
        popup.set_has_result(false);
        popup.set_error_title(title.into());
        popup.set_error_message(message.into());
        popup.set_error_show_settings(needs_settings);
        popup.set_error_visible(true);
    }

    pub fn select_language(&self, code: &str) {
        let changed = {
            let mut state = self.state();
            let Some(code) = state
                .find_language(code)
                .map(|language| language.code.clone())
            else {
                return;
            };
            let changed = !state.target_language().code.eq_ignore_ascii_case(&code);
            state.add_suggested(&code);
            state.save_preferences();
            changed
        };
        if let Some(popup) = self.popup.upgrade() {
            popup.set_language_search("".into());
        }
        self.refresh_language_ui();
        if changed {
            self.begin_translation(false);
        }
    }

    /// Enter in the search box picks the first language that matches.
    pub fn select_first_match(&self) {
        let Some(popup) = self.popup.upgrade() else {
            return;
        };
        let first_match = {
            let state = self.state();
            let (recommended, all) = state::language_items(&state, &popup.get_language_search());
            recommended.into_iter().chain(all).next()
        };
        if let Some(language) = first_match {
            popup.invoke_reset_view();
            self.fit_window_to_menu();
            self.select_language(&language.code);
        }
    }

    pub fn refresh_language_ui(&self) {
        let Some(popup) = self.popup.upgrade() else {
            return;
        };
        let (recommended, all, code, name) = {
            let state = self.state();
            let (recommended, all) = state::language_items(&state, &popup.get_language_search());
            let target = state.target_language();
            (
                recommended,
                all,
                target.code.clone(),
                target.display_name.clone(),
            )
        };
        popup.set_recommended_languages(ModelRc::new(VecModel::from(recommended)));
        popup.set_all_languages(ModelRc::new(VecModel::from(all)));
        popup.set_selected_language_code(code.into());
        popup.set_selected_language_name(name.into());
    }

    pub fn copy_translation(&self) {
        let Some(popup) = self.popup.upgrade() else {
            return;
        };
        let Some(text) = self.state().translation().map(str::to_owned) else {
            return;
        };
        let Some(owner_window) = placement::native_window_handle(popup.window()) else {
            eprintln!("failed to copy translation: clipboard owner window is unavailable");
            return;
        };
        match windows_integration::copy_text(owner_window, &text) {
            Ok(()) => self.hide_popup(),
            Err(error) => eprintln!("failed to copy translation: {error}"),
        }
    }

    pub fn replace_selection(&self) {
        let Some(popup) = self.popup.upgrade() else {
            return;
        };
        let Some(owner_window) = placement::native_window_handle(popup.window()) else {
            self.show_error(
                deepl::TRANSLATE_ERROR_TITLE,
                "클립보드를 사용할 수 없습니다.",
                false,
            );
            return;
        };
        let (source_window, translation, anchor) = {
            let state = self.state();
            let Some(translation) = state.translation().map(str::to_owned) else {
                return;
            };
            if state.source_window == 0 {
                return;
            }
            (state.source_window, translation, state.last_anchor)
        };

        self.hide_popup();
        let app = self.clone();
        thread::spawn(move || {
            if let Err(error) = windows_integration::replace_selected_text(
                owner_window,
                source_window,
                &translation,
            ) {
                // Like the original, bring the popup back with the translation
                // and say briefly why replacing did not work.
                let _ = slint::invoke_from_event_loop(move || {
                    app.show_popup(anchor);
                    if let Some(popup) = app.popup.upgrade() {
                        popup.set_notification(error.into());
                    }
                });
            }
        });
    }

    /// Shows the popup from the tray, with the placeholder when nothing was selected.
    pub fn open_from_tray(&self) {
        let Some(popup) = self.popup.upgrade() else {
            return;
        };
        if self.state().source_text.is_empty() {
            popup.set_error_visible(false);
            popup.set_result_text("".into());
        }
        self.show_popup(windows_integration::current_anchor());
    }

    pub fn show_popup(&self, anchor: PopupAnchor) {
        let Some(popup) = self.popup.upgrade() else {
            return;
        };
        {
            let mut state = self.state();
            state.last_anchor = anchor;
            state.menu_shift = 0;
        }
        self.apply_theme();
        popup.set_menu_extra(0.0);
        popup.window().set_size(placement::popup_size());
        placement::position_popup(popup.window(), anchor);
        if let Err(error) = popup.show() {
            eprintln!("failed to show popup: {error}");
            return;
        }
        // Showing can move the window to a monitor with another scale factor,
        // which changes its physical size, so place it once more.
        placement::position_popup(popup.window(), anchor);
        placement::activate_window(popup.window());
        popup.invoke_reset_view();
        self.sync_watchers();
    }

    pub fn hide_popup(&self) {
        let Some(popup) = self.popup.upgrade() else {
            return;
        };
        popup.invoke_reset_view();
        popup.set_menu_extra(0.0);
        let _ = popup.hide();
        self.sync_watchers();
    }

    pub fn close_popup_if_inactive(&self) {
        let Some(popup) = self.popup.upgrade() else {
            return;
        };
        if popup.window().is_visible() && !placement::window_has_focus(popup.window()) {
            self.hide_popup();
        }
    }

    /// The language list may need more room than the card has. The window then
    /// grows with transparent space below the card and moves up if needed.
    pub fn fit_window_to_menu(&self) {
        let Some(popup) = self.popup.upgrade() else {
            return;
        };
        let window = popup.window();
        let scale_factor = window.scale_factor();
        let size = window.size().to_logical(scale_factor);
        let base_height = size.height - popup.get_menu_extra();
        let mut position = window.position();
        let fit = {
            let mut state = self.state();
            // Undo the upward shift of a previous opening first.
            position.y += std::mem::take(&mut state.menu_shift);
            let needed = if popup.get_menu_open() {
                popup.get_menu_bottom() - base_height
            } else {
                0.0
            };
            let fit = placement::fit_menu(
                needed,
                base_height,
                position.y,
                state.last_anchor.work_area,
                scale_factor,
            );
            state.menu_shift = fit.shift;
            fit
        };
        position.y -= fit.shift;

        popup.set_menu_extra(fit.extra);
        window.set_position(position);
        window.set_size(LogicalSize::new(size.width, base_height + fit.extra));
    }

    pub fn open_settings(&self, anchor: PopupAnchor, message: Option<&str>) {
        let Some(settings_window) = self.settings.upgrade() else {
            return;
        };
        {
            let state = self.state();
            settings_window.set_api_key(state.api_key.as_str().into());
            settings_window.set_free_api(state.use_free_api);
        }
        settings_window.set_launch_at_startup(windows_integration::launch_at_startup());
        settings_window.set_message(message.unwrap_or_default().into());
        self.apply_theme();
        if let Err(error) = settings_window.show() {
            eprintln!("failed to show settings window: {error}");
            return;
        }
        placement::center_in_work_area(settings_window.window(), anchor.work_area);
        placement::activate_window(settings_window.window());
        settings_window.invoke_focus_key();
        self.sync_watchers();
    }

    pub fn hide_settings(&self) {
        if let Some(settings_window) = self.settings.upgrade() {
            let _ = settings_window.hide();
        }
        self.sync_watchers();
    }

    pub fn save_settings(&self, api_key: &str, free_api: bool, launch_at_startup: bool) {
        let Some(settings_window) = self.settings.upgrade() else {
            return;
        };
        if api_key.is_empty() {
            settings_window.set_message("DeepL API 인증 키를 입력하세요.".into());
            return;
        }
        // Rewriting the entry also keeps its path current if the app was moved.
        if let Err(error) = windows_integration::set_launch_at_startup(launch_at_startup) {
            settings_window.set_message(error.into());
            return;
        }

        let (previous, candidate) = {
            let state = self.state();
            let previous = state.preferences();
            let candidate = Preferences::new(
                free_api,
                previous.suggested_languages.clone(),
                previous.target_languages.clone(),
            );
            (previous, candidate)
        };
        if let Err(error) = candidate.save() {
            settings_window.set_message(error.into());
            return;
        }
        if let Err(error) = settings::save_api_key(api_key) {
            let message = match previous.save() {
                Ok(()) => error,
                Err(rollback_error) => {
                    format!("{error} 이전 설정도 복원하지 못했습니다: {rollback_error}")
                }
            };
            settings_window.set_message(message.into());
            return;
        }

        let (retranslate, anchor) = {
            let mut state = self.state();
            if state.api_key != api_key || state.use_free_api != free_api {
                state.clear_translations();
            }
            state.api_key = api_key.to_owned();
            state.use_free_api = free_api;
            (
                !state.source_text.is_empty() && state.translation().is_none(),
                state.last_anchor,
            )
        };
        settings_window.set_message("".into());
        self.hide_settings();
        self.refresh_target_languages();

        if retranslate {
            self.show_popup(anchor);
            self.begin_translation(true);
        }
    }

    pub fn refresh_target_languages(&self) {
        let (api_key, use_free_api) = {
            let state = self.state();
            (state.api_key.clone(), state.use_free_api)
        };
        if api_key.trim().is_empty() {
            return;
        }

        let app = self.clone();
        thread::spawn(move || {
            let languages = match deepl::target_languages(&api_key, use_free_api) {
                Ok(languages) => languages,
                Err(error) => {
                    eprintln!("failed to retrieve DeepL target languages: {error}");
                    return;
                }
            };
            let _ = slint::invoke_from_event_loop(move || {
                {
                    let mut state = app.state();
                    if state.api_key != api_key || state.use_free_api != use_free_api {
                        return;
                    }
                    state.set_languages(languages);
                    state.save_preferences();
                }
                app.refresh_language_ui();
            });
        });
    }

    /// Follows the Windows theme: the app theme for the windows and the taskbar
    /// theme for the tray icon.
    pub fn apply_theme(&self) {
        if let Some(tray) = self.tray.upgrade() {
            let dark_taskbar = windows_integration::windows_system_dark_mode();
            if tray.get_dark_taskbar() != dark_taskbar {
                tray.set_dark_taskbar(dark_taskbar);
            }
            let icon_size = windows_integration::small_icon_size();
            if tray.get_icon_size() != icon_size {
                tray.set_icon_size(icon_size);
            }
        }
        let dark_mode = windows_integration::windows_app_dark_mode();
        if let Some(popup) = self.popup.upgrade()
            && popup.get_dark_mode() != dark_mode
        {
            popup.set_dark_mode(dark_mode);
        }
        if let Some(settings_window) = self.settings.upgrade()
            && settings_window.get_dark_mode() != dark_mode
        {
            settings_window.set_dark_mode(dark_mode);
        }
    }

    fn visible_windows(&self) -> (bool, bool) {
        let popup_visible = self
            .popup
            .upgrade()
            .is_some_and(|popup| popup.window().is_visible());
        let settings_visible = self
            .settings
            .upgrade()
            .is_some_and(|settings_window| settings_window.window().is_visible());
        (popup_visible, settings_visible)
    }

    /// Polls for outside clicks only while the popup is open. Once no window is
    /// shown, the memory the renderer keeps is released to the OS.
    fn sync_watchers(&self) {
        let (popup_visible, settings_visible) = self.visible_windows();

        if !popup_visible && !settings_visible {
            let app = self.clone();
            Timer::single_shot(TRIM_DELAY, move || {
                if app.visible_windows() == (false, false) {
                    windows_integration::trim_working_set();
                }
            });
        }

        OUTSIDE_CLICK_TIMER.with(|timer| {
            if !popup_visible {
                timer.stop();
            } else if !timer.running() {
                // Fallback for when Windows refuses to give the popup focus: a
                // click outside the popup still closes it.
                let app = self.clone();
                let was_down = Cell::new(windows_integration::left_button_down());
                timer.start(
                    TimerMode::Repeated,
                    OUTSIDE_CLICK_POLL_INTERVAL,
                    move || {
                        let is_down = windows_integration::left_button_down();
                        if is_down
                            && !was_down.get()
                            && let Some(popup) = app.popup.upgrade()
                            && !placement::cursor_is_inside(popup.window())
                        {
                            app.hide_popup();
                        }
                        was_down.set(is_down);
                    },
                );
            }
        });
    }
}

//! Translation state shared between the UI thread and request threads.

use std::collections::HashMap;

use crate::LanguageItem;
use crate::languages::{self, TargetLanguage};
use crate::settings::Preferences;
use crate::windows_integration::PopupAnchor;

pub const MAX_SUGGESTED_LANGUAGES: usize = 6;
const DEFAULT_SUGGESTED_LANGUAGES: [&str; 2] = ["KO", "EN-US"];

#[derive(Debug)]
pub struct AppState {
    pub api_key: String,
    pub use_free_api: bool,
    /// Target languages sorted by display name. Never empty.
    languages: Vec<TargetLanguage>,
    /// Recently chosen target languages, most recent first, spelled like in
    /// `languages`. The first one is the target.
    suggested: Vec<String>,
    pub source_text: String,
    pub source_window: isize,
    /// Translations of `source_text` keyed by lowercase target code.
    translations: HashMap<String, String>,
    pub last_anchor: PopupAnchor,
    /// Pixels the popup was moved up to make room for the open language list.
    pub menu_shift: i32,
}

impl AppState {
    pub fn new(api_key: String, preferences: Preferences, anchor: PopupAnchor) -> Self {
        let languages = if preferences.target_languages.is_empty() {
            languages::fallback_languages()
        } else {
            languages::prepare(preferences.target_languages)
        };
        let mut state = Self {
            api_key,
            use_free_api: preferences.use_free_api,
            languages,
            suggested: preferences.suggested_languages,
            source_text: String::new(),
            source_window: 0,
            translations: HashMap::new(),
            last_anchor: anchor,
            menu_shift: 0,
        };
        state.canonicalize_suggested();
        state
    }

    pub fn set_languages(&mut self, languages: Vec<TargetLanguage>) {
        let languages = languages::prepare(languages);
        if !languages.is_empty() {
            self.languages = languages;
            self.canonicalize_suggested();
        }
    }

    pub fn find_language(&self, code: &str) -> Option<&TargetLanguage> {
        languages::find(&self.languages, code)
    }

    pub fn target_language(&self) -> &TargetLanguage {
        self.suggested
            .iter()
            .find_map(|code| self.find_language(code))
            .unwrap_or(&self.languages[0])
    }

    fn is_suggested(&self, code: &str) -> bool {
        self.suggested
            .iter()
            .any(|suggested| suggested.eq_ignore_ascii_case(code))
    }

    /// Suggested languages, then the rest by display name: the dropdown order.
    fn ordered_languages(&self) -> (Vec<&TargetLanguage>, Vec<&TargetLanguage>) {
        let suggested = self
            .suggested
            .iter()
            .filter_map(|code| self.find_language(code))
            .collect();
        let others = self
            .languages
            .iter()
            .filter(|language| !self.is_suggested(&language.code))
            .collect();
        (suggested, others)
    }

    pub fn add_suggested(&mut self, code: &str) {
        self.suggested
            .retain(|existing| !existing.eq_ignore_ascii_case(code));
        self.suggested.insert(0, code.to_owned());
        self.suggested.truncate(MAX_SUGGESTED_LANGUAGES);
    }

    /// The target to use when the text is already in the target language: the
    /// first language in list order that differs from the source language.
    pub fn fallback_target(&self, source_language: &str) -> &TargetLanguage {
        let target = self.target_language();
        if !languages::same_family(source_language, &target.code) {
            return target;
        }
        let (suggested, others) = self.ordered_languages();
        suggested
            .into_iter()
            .chain(others)
            .find(|language| !languages::same_family(source_language, &language.code))
            .or_else(|| {
                let code = if languages::family(source_language) == "en" {
                    "DE"
                } else {
                    "EN-GB"
                };
                self.find_language(code)
            })
            .unwrap_or(target)
    }

    /// Keeps only suggested languages that exist in the language list, using
    /// the list's spelling of each code.
    fn canonicalize_suggested(&mut self) {
        let known = |codes: Vec<&str>| {
            let mut kept: Vec<String> = Vec::new();
            for code in codes {
                if let Some(language) = languages::find(&self.languages, code)
                    && !kept
                        .iter()
                        .any(|existing| existing.eq_ignore_ascii_case(&language.code))
                {
                    kept.push(language.code.clone());
                }
            }
            kept
        };
        let mut canonical = known(self.suggested.iter().map(String::as_str).collect());
        if canonical.is_empty() {
            canonical = known(DEFAULT_SUGGESTED_LANGUAGES.to_vec());
        }
        if canonical.is_empty() {
            canonical.push(self.languages[0].code.clone());
        }
        self.suggested = canonical;
    }

    pub fn set_source(&mut self, text: String, window: isize) {
        self.source_text = text;
        self.source_window = window;
        self.translations.clear();
    }

    pub fn clear_source(&mut self) {
        self.set_source(String::new(), 0);
    }

    /// The translation into the current target language, if there is one.
    pub fn translation(&self) -> Option<&str> {
        self.translations
            .get(&self.target_language().code.to_ascii_lowercase())
            .map(String::as_str)
    }

    pub fn store_translation(&mut self, target_code: &str, text: String) {
        self.translations
            .insert(target_code.to_ascii_lowercase(), text);
    }

    pub fn clear_translations(&mut self) {
        self.translations.clear();
    }

    pub fn preferences(&self) -> Preferences {
        Preferences::new(
            self.use_free_api,
            self.suggested.clone(),
            self.languages.clone(),
        )
    }

    pub fn save_preferences(&self) {
        if let Err(error) = self.preferences().save() {
            eprintln!("failed to save preferences: {error}");
        }
    }
}

/// The dropdown entries filtered by `query` like the original: names
/// containing the query, earlier matches first.
pub fn language_items(state: &AppState, query: &str) -> (Vec<LanguageItem>, Vec<LanguageItem>) {
    let query = query.trim().to_lowercase();
    let filter = |languages: Vec<&TargetLanguage>| {
        let mut matches = languages
            .into_iter()
            .filter_map(|language| {
                let position = if query.is_empty() {
                    Some(0)
                } else {
                    language
                        .display_name
                        .to_lowercase()
                        .find(&query)
                        .or_else(|| language.name.to_lowercase().find(&query))
                };
                position.map(|position| {
                    let item = LanguageItem {
                        code: language.code.as_str().into(),
                        name: language.display_name.as_str().into(),
                    };
                    (position, item)
                })
            })
            .collect::<Vec<_>>();
        matches.sort_by_key(|(position, _)| *position);
        matches
            .into_iter()
            .map(|(_, item)| item)
            .collect::<Vec<_>>()
    };
    let (suggested, others) = state.ordered_languages();
    (filter(suggested), filter(others))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows_integration::WorkArea;

    fn state(suggested: &[&str]) -> AppState {
        let languages = [
            ("de", "German"),
            ("en-GB", "English (British)"),
            ("en-US", "English (American)"),
            ("ja", "Japanese"),
            ("ko", "Korean"),
        ]
        .into_iter()
        .map(|(code, name)| TargetLanguage::new(code, name))
        .collect();
        let preferences = Preferences::new(
            true,
            suggested.iter().map(|code| (*code).to_owned()).collect(),
            languages,
        );
        let anchor = PopupAnchor {
            cursor_x: 0,
            cursor_y: 0,
            work_area: WorkArea {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
                scale_factor: 1.0,
            },
        };
        AppState::new(String::new(), preferences, anchor)
    }

    #[test]
    fn suggested_languages_use_the_list_spelling() {
        let state = state(&["KO", "EN-US", "XX"]);
        assert_eq!(state.suggested, ["ko", "en-US"]);
        assert_eq!(state.target_language().code, "ko");
    }

    #[test]
    fn unknown_suggestions_fall_back_to_korean_and_english() {
        let state = state(&["XX"]);
        assert_eq!(state.suggested, ["ko", "en-US"]);
    }

    #[test]
    fn chosen_language_moves_to_the_front_of_the_suggestions() {
        let mut state = state(&["ko", "en-US", "de", "ja", "en-GB"]);
        state.add_suggested("ja");
        assert_eq!(state.suggested, ["ja", "ko", "en-US", "de", "en-GB"]);

        state.add_suggested("fr");
        state.add_suggested("it");
        assert_eq!(state.suggested.len(), MAX_SUGGESTED_LANGUAGES);
        assert_eq!(state.suggested[0], "it");
    }

    #[test]
    fn text_in_the_target_language_switches_to_the_next_suggestion() {
        let state = state(&["ko", "en-US"]);
        assert_eq!(state.fallback_target("KO").code, "en-US");
        assert_eq!(state.fallback_target("JA").code, "ko");

        let state = self::state(&["en-US", "en-GB", "ko"]);
        assert_eq!(state.fallback_target("EN").code, "ko");
    }

    #[test]
    fn translations_follow_the_target_language() {
        let mut state = state(&["ko", "en-US"]);
        state.set_source("Hello".to_owned(), 1);
        state.store_translation("KO", "안녕".to_owned());
        assert_eq!(state.translation(), Some("안녕"));

        state.add_suggested("en-US");
        assert_eq!(state.translation(), None);

        state.add_suggested("ko");
        assert_eq!(state.translation(), Some("안녕"));
        state.set_source("Bye".to_owned(), 1);
        assert_eq!(state.translation(), None);
    }

    #[test]
    fn search_matches_names_and_orders_by_match_position() {
        let state = state(&["ko", "en-US"]);
        let (recommended, all) = language_items(&state, "영어");
        assert_eq!(recommended.len(), 1);
        assert_eq!(recommended[0].name, "영어(미국)");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "영어(영국)");

        let (recommended, all) = language_items(&state, "german");
        assert!(recommended.is_empty());
        assert_eq!(all[0].name, "독일어");
    }

    #[test]
    fn other_languages_are_sorted_by_displayed_name() {
        let state = state(&["ko"]);
        let (_, all) = language_items(&state, "");
        let names = all
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["독일어", "영어(미국)", "영어(영국)", "일본어"]);
    }
}

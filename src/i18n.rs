//! The language of the app's own text. Slint strings are translated through
//! the bundled gettext catalogs in `lang/`; Rust strings go through `messages`.

use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Korean,
}

/// The interface language chosen in settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterfaceLanguage {
    /// Follows the Windows display language.
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "ko")]
    Korean,
}

impl InterfaceLanguage {
    /// The order of the choices in the settings window.
    pub const ALL: [Self; 3] = [Self::Auto, Self::English, Self::Korean];

    pub fn resolve(self) -> Language {
        match self {
            Self::Auto => system_language(),
            Self::English => Language::English,
            Self::Korean => Language::Korean,
        }
    }

    pub fn index(self) -> i32 {
        Self::ALL
            .iter()
            .position(|choice| *choice == self)
            .unwrap_or(0) as i32
    }

    pub fn from_index(index: i32) -> Self {
        usize::try_from(index)
            .ok()
            .and_then(|index| Self::ALL.get(index).copied())
            .unwrap_or_default()
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn current() -> Language {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Language::Korean,
        _ => Language::English,
    }
}

/// Switches the language of both Rust and Slint text. Slint updates every
/// visible `@tr` string right away.
pub fn set(language: Language) {
    CURRENT.store(
        match language {
            Language::English => 0,
            Language::Korean => 1,
        },
        Ordering::Relaxed,
    );
    // English is the source language, so it has no catalog of its own and
    // selecting it fails; the failure leaves the source strings in place.
    let code = match language {
        Language::English => "en",
        Language::Korean => "ko",
    };
    let _ = slint::select_bundled_translation(code);
}

#[cfg(windows)]
fn system_language() -> Language {
    use windows::Win32::Globalization::GetUserDefaultUILanguage;

    const LANG_KOREAN: u16 = 0x12;
    // The low ten bits of a LANGID are the primary language.
    if unsafe { GetUserDefaultUILanguage() } & 0x3ff == LANG_KOREAN {
        Language::Korean
    } else {
        Language::English
    }
}

#[cfg(not(windows))]
fn system_language() -> Language {
    Language::English
}

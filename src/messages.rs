//! Text that Rust code shows to users, in every interface language. English is
//! the source; Korean is the translation.

use crate::i18n::{self, Language};

pub enum Message {
    // Popup error titles.
    TranslateFailedTitle,
    CheckApiKeyTitle,
    QuotaReachedTitle,
    CaptureFailedTitle,
    ApiKeyRequiredTitle,
    HotkeyUnavailableTitle,

    // Translation errors.
    NoSelectedText,
    TextTooLong,
    ConnectionFailed {
        message: String,
    },
    InvalidApiKey,
    TooManyRequests,
    QuotaExceeded,
    ServerError {
        status: u16,
        message: String,
    },
    UnreadableResponse,

    // Popup and settings window.
    Placeholder,
    EnterApiKeyToTranslate,
    EnterApiKey,
    ClipboardUnavailable,
    RollbackFailed {
        error: String,
        rollback_error: String,
    },

    // Selection, replacement and startup.
    HotkeyRegistrationFailed {
        error: String,
    },
    NoActiveWindow,
    SelectionNotText,
    SelectionUnavailable,
    NoTranslationToReplace,
    SourceWindowClosed,
    SourceWindowUnreachable,
    SourceReadOnly,
    KeyboardInputFailed,
    /// `error` is the system's description, or empty when there is none.
    ClipboardFailed {
        error: String,
    },
    LaunchAtStartupFailed,
    #[cfg_attr(windows, allow(dead_code))]
    LaunchAtStartupUnsupported,

    // Settings file and Windows Credential Manager.
    SettingsFolderFailed {
        error: String,
    },
    SettingsSerializeFailed {
        error: String,
    },
    SettingsSaveFailed {
        error: String,
    },
    ApiKeyReadFailed {
        error: String,
    },
    InvalidCredential,
    StoredApiKeyInvalid,
    ApiKeyStoreFailed {
        error: String,
    },
    #[cfg_attr(windows, allow(dead_code))]
    ApiKeyStorageUnsupported,
}

impl Message {
    /// The text in the current interface language.
    pub fn text(&self) -> String {
        self.text_in(i18n::current())
    }

    pub fn text_in(&self, language: Language) -> String {
        match language {
            Language::English => self.english(),
            Language::Korean => self.korean(),
        }
    }

    fn english(&self) -> String {
        match self {
            Self::TranslateFailedTitle => "Couldn't translate the text".to_owned(),
            Self::CheckApiKeyTitle => "Check your DeepL API key".to_owned(),
            Self::QuotaReachedTitle => "DeepL API usage limit reached".to_owned(),
            Self::CaptureFailedTitle => "Couldn't get the text".to_owned(),
            Self::ApiKeyRequiredTitle => "DeepL API key required".to_owned(),
            Self::HotkeyUnavailableTitle => "Shortcut unavailable".to_owned(),

            Self::NoSelectedText => "No text is selected.".to_owned(),
            Self::TextTooLong => {
                "The selected text is too long. Split it up and translate it in parts.".to_owned()
            }
            Self::ConnectionFailed { message } => {
                format!("Couldn't connect to DeepL. Try again later. ({message})")
            }
            Self::InvalidApiKey => {
                "In settings, check that the API key and plan (Free/Pro) match.".to_owned()
            }
            Self::TooManyRequests => {
                "Too many requests in a short time. Try again in a few minutes.".to_owned()
            }
            Self::QuotaExceeded => {
                "You've used up your DeepL API quota for this billing period.".to_owned()
            }
            Self::ServerError { status, message } if message.is_empty() => {
                format!("Try again later. (HTTP {status})")
            }
            Self::ServerError { status, message } => {
                format!("Try again later. ({message}, HTTP {status})")
            }
            Self::UnreadableResponse => {
                "Couldn't read the response from DeepL. Try again later.".to_owned()
            }

            Self::Placeholder => "Select some text, then press Ctrl+F9.".to_owned(),
            Self::EnterApiKeyToTranslate => "Enter your DeepL API key to translate.".to_owned(),
            Self::EnterApiKey => "Enter your DeepL API key.".to_owned(),
            Self::ClipboardUnavailable => "The clipboard isn't available.".to_owned(),
            Self::RollbackFailed {
                error,
                rollback_error,
            } => format!(
                "{error} The previous settings couldn't be restored either: {rollback_error}"
            ),

            Self::HotkeyRegistrationFailed { error } => format!(
                "Couldn't register the Ctrl+F9 shortcut. Check whether another app, such as DeepL, is using it. ({error})"
            ),
            Self::NoActiveWindow => "Couldn't find the active window.".to_owned(),
            Self::SelectionNotText => "The selection isn't text.".to_owned(),
            Self::SelectionUnavailable => {
                "Couldn't get the selected text. Select some text and try again.".to_owned()
            }
            Self::NoTranslationToReplace => {
                "There's no translation to replace the original with.".to_owned()
            }
            Self::SourceWindowClosed => {
                "The window with the original text was closed, so it can't be replaced.".to_owned()
            }
            Self::SourceWindowUnreachable => {
                "Couldn't switch back to the window with the original text.".to_owned()
            }
            Self::SourceReadOnly => {
                "The original text can't be edited, so it can't be replaced.".to_owned()
            }
            Self::KeyboardInputFailed => {
                "Couldn't send keystrokes to the window with the original text.".to_owned()
            }
            Self::ClipboardFailed { error } if error.is_empty() => {
                "Couldn't use the clipboard.".to_owned()
            }
            Self::ClipboardFailed { error } => format!("Couldn't use the clipboard: {error}"),
            Self::LaunchAtStartupFailed => {
                "Couldn't change the launch at startup setting.".to_owned()
            }
            Self::LaunchAtStartupUnsupported => {
                "Launch at startup isn't supported on this platform.".to_owned()
            }

            Self::SettingsFolderFailed { error } => {
                format!("Couldn't create the settings folder: {error}")
            }
            Self::SettingsSerializeFailed { error } => {
                format!("Couldn't prepare the settings for saving: {error}")
            }
            Self::SettingsSaveFailed { error } => format!("Couldn't save the settings: {error}"),
            Self::ApiKeyReadFailed { error } => {
                format!("Couldn't read the API key from Windows Credential Manager: {error}")
            }
            Self::InvalidCredential => {
                "Windows Credential Manager returned an invalid credential.".to_owned()
            }
            Self::StoredApiKeyInvalid => "The saved DeepL API key is invalid.".to_owned(),
            Self::ApiKeyStoreFailed { error } => {
                format!("Couldn't save the API key to Windows Credential Manager: {error}")
            }
            Self::ApiKeyStorageUnsupported => {
                "Saving the API key is supported only on Windows.".to_owned()
            }
        }
    }

    fn korean(&self) -> String {
        match self {
            Self::TranslateFailedTitle => "텍스트를 번역할 수 없습니다".to_owned(),
            Self::CheckApiKeyTitle => "DeepL API 인증 키를 확인하세요".to_owned(),
            Self::QuotaReachedTitle => "DeepL API 사용 한도에 도달했습니다".to_owned(),
            Self::CaptureFailedTitle => "텍스트를 가져올 수 없습니다".to_owned(),
            Self::ApiKeyRequiredTitle => "DeepL API 인증 키가 필요합니다".to_owned(),
            Self::HotkeyUnavailableTitle => "단축키를 사용할 수 없습니다".to_owned(),

            Self::NoSelectedText => "선택한 텍스트가 없습니다.".to_owned(),
            Self::TextTooLong => {
                "선택한 텍스트가 너무 깁니다. 더 짧게 나눠서 번역하세요.".to_owned()
            }
            Self::ConnectionFailed { message } => {
                format!("DeepL에 연결하지 못했습니다. 나중에 다시 시도하세요. ({message})")
            }
            Self::InvalidApiKey => {
                "설정에서 인증 키와 API 플랜(Free/Pro)이 맞는지 확인하세요.".to_owned()
            }
            Self::TooManyRequests => {
                "단기간에 너무 많은 요청을 보냈습니다. 몇 분 후에 다시 시도하세요.".to_owned()
            }
            Self::QuotaExceeded => {
                "이번 결제 주기의 DeepL API 사용량을 모두 사용했습니다.".to_owned()
            }
            Self::ServerError { status, message } if message.is_empty() => {
                format!("나중에 다시 시도하세요. (HTTP {status})")
            }
            Self::ServerError { status, message } => {
                format!("나중에 다시 시도하세요. ({message}, HTTP {status})")
            }
            Self::UnreadableResponse => {
                "DeepL 응답을 읽지 못했습니다. 나중에 다시 시도하세요.".to_owned()
            }

            Self::Placeholder => "텍스트를 선택한 다음 Ctrl+F9를 누르세요.".to_owned(),
            Self::EnterApiKeyToTranslate => "번역하려면 DeepL API 인증 키를 입력하세요.".to_owned(),
            Self::EnterApiKey => "DeepL API 인증 키를 입력하세요.".to_owned(),
            Self::ClipboardUnavailable => "클립보드를 사용할 수 없습니다.".to_owned(),
            Self::RollbackFailed {
                error,
                rollback_error,
            } => format!("{error} 이전 설정도 복원하지 못했습니다: {rollback_error}"),

            Self::HotkeyRegistrationFailed { error } => format!(
                "Ctrl+F9 단축키를 등록하지 못했습니다. 다른 앱(DeepL 등)이 사용 중인지 확인하세요. ({error})"
            ),
            Self::NoActiveWindow => "활성 창을 찾지 못했습니다.".to_owned(),
            Self::SelectionNotText => "선택한 내용이 텍스트가 아닙니다.".to_owned(),
            Self::SelectionUnavailable => {
                "선택한 텍스트를 가져오지 못했습니다. 텍스트를 선택한 다음 다시 시도하세요."
                    .to_owned()
            }
            Self::NoTranslationToReplace => "원문을 대체할 번역문이 없습니다.".to_owned(),
            Self::SourceWindowClosed => "원문이 있던 창이 닫혀서 대체할 수 없습니다.".to_owned(),
            Self::SourceWindowUnreachable => "원문이 있던 창으로 돌아가지 못했습니다.".to_owned(),
            Self::SourceReadOnly => "원문은 편집할 수 없으므로 대체할 수 없습니다.".to_owned(),
            Self::KeyboardInputFailed => "원문이 있던 창에 키 입력을 보내지 못했습니다.".to_owned(),
            Self::ClipboardFailed { error } if error.is_empty() => {
                "클립보드를 사용하지 못했습니다.".to_owned()
            }
            Self::ClipboardFailed { error } => format!("클립보드를 사용하지 못했습니다: {error}"),
            Self::LaunchAtStartupFailed => "시작 시 자동 실행 설정을 바꾸지 못했습니다.".to_owned(),
            Self::LaunchAtStartupUnsupported => {
                "이 플랫폼에서는 시작 시 자동 실행을 지원하지 않습니다.".to_owned()
            }

            Self::SettingsFolderFailed { error } => {
                format!("설정 폴더를 만들지 못했습니다: {error}")
            }
            Self::SettingsSerializeFailed { error } => {
                format!("설정을 저장할 형식으로 바꾸지 못했습니다: {error}")
            }
            Self::SettingsSaveFailed { error } => format!("설정을 저장하지 못했습니다: {error}"),
            Self::ApiKeyReadFailed { error } => {
                format!("Windows 자격 증명 관리자에서 API 인증 키를 읽지 못했습니다: {error}")
            }
            Self::InvalidCredential => {
                "Windows 자격 증명 관리자가 올바르지 않은 자격 증명을 반환했습니다.".to_owned()
            }
            Self::StoredApiKeyInvalid => "저장된 DeepL API 인증 키가 올바르지 않습니다.".to_owned(),
            Self::ApiKeyStoreFailed { error } => {
                format!("Windows 자격 증명 관리자에 API 인증 키를 저장하지 못했습니다: {error}")
            }
            Self::ApiKeyStorageUnsupported => {
                "API 인증 키 저장은 Windows에서만 지원합니다.".to_owned()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_message() -> Vec<Message> {
        let error = || "error".to_owned();
        vec![
            Message::TranslateFailedTitle,
            Message::CheckApiKeyTitle,
            Message::QuotaReachedTitle,
            Message::CaptureFailedTitle,
            Message::ApiKeyRequiredTitle,
            Message::HotkeyUnavailableTitle,
            Message::NoSelectedText,
            Message::TextTooLong,
            Message::ConnectionFailed { message: error() },
            Message::InvalidApiKey,
            Message::TooManyRequests,
            Message::QuotaExceeded,
            Message::ServerError {
                status: 500,
                message: String::new(),
            },
            Message::ServerError {
                status: 500,
                message: error(),
            },
            Message::UnreadableResponse,
            Message::Placeholder,
            Message::EnterApiKeyToTranslate,
            Message::EnterApiKey,
            Message::ClipboardUnavailable,
            Message::RollbackFailed {
                error: error(),
                rollback_error: error(),
            },
            Message::HotkeyRegistrationFailed { error: error() },
            Message::NoActiveWindow,
            Message::SelectionNotText,
            Message::SelectionUnavailable,
            Message::NoTranslationToReplace,
            Message::SourceWindowClosed,
            Message::SourceWindowUnreachable,
            Message::SourceReadOnly,
            Message::KeyboardInputFailed,
            Message::ClipboardFailed {
                error: String::new(),
            },
            Message::ClipboardFailed { error: error() },
            Message::LaunchAtStartupFailed,
            Message::LaunchAtStartupUnsupported,
            Message::SettingsFolderFailed { error: error() },
            Message::SettingsSerializeFailed { error: error() },
            Message::SettingsSaveFailed { error: error() },
            Message::ApiKeyReadFailed { error: error() },
            Message::InvalidCredential,
            Message::StoredApiKeyInvalid,
            Message::ApiKeyStoreFailed { error: error() },
            Message::ApiKeyStorageUnsupported,
        ]
    }

    #[test]
    fn every_message_has_text_in_both_languages() {
        for message in every_message() {
            let english = message.text_in(Language::English);
            let korean = message.text_in(Language::Korean);
            assert!(!english.trim().is_empty());
            assert!(!korean.trim().is_empty());
            assert_ne!(english, korean);
        }
    }

    #[test]
    fn parameters_appear_in_the_text() {
        let message = Message::ServerError {
            status: 503,
            message: "busy".to_owned(),
        };
        for language in [Language::English, Language::Korean] {
            let text = message.text_in(language);
            assert!(text.contains("busy") && text.contains("503"));
        }
    }
}

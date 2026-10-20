use std::fmt;
use std::sync::OnceLock;
use std::time::Duration;

use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};

use crate::languages::TargetLanguage;
use crate::messages::Message;

const FREE_API_BASE: &str = "https://api-free.deepl.com";
const PRO_API_BASE: &str = "https://api.deepl.com";
const TRANSLATE_PATH: &str = "/v2/translate";
const LANGUAGES_PATH: &str = "/v3/languages?resource=translate_text";
const TRANSLATE_TIMEOUT: Duration = Duration::from_secs(30);
const LANGUAGES_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_TEXT_BYTES: usize = 120 * 1024;

#[derive(Debug, Clone)]
pub struct Translation {
    pub text: String,
    pub detected_source_language: Option<String>,
}

#[derive(Debug)]
pub enum TranslateError {
    EmptyText,
    TextTooLarge,
    Request(String),
    Api { status: u16, message: String },
    InvalidResponse,
}

/// How an error is presented in the popup: a title, a description and whether
/// the fix is in the settings window.
pub struct ErrorPresentation {
    pub title: String,
    pub description: String,
    pub needs_settings: bool,
}

impl TranslateError {
    pub fn presentation(&self) -> ErrorPresentation {
        let (title, needs_settings) = match self {
            Self::Api {
                status: 401 | 403, ..
            } => (Message::CheckApiKeyTitle, true),
            Self::Api { status: 456, .. } => (Message::QuotaReachedTitle, false),
            _ => (Message::TranslateFailedTitle, false),
        };
        ErrorPresentation {
            title: title.text(),
            description: self.to_string(),
            needs_settings,
        }
    }

    fn message(&self) -> Message {
        match self {
            Self::EmptyText => Message::NoSelectedText,
            Self::TextTooLarge => Message::TextTooLong,
            Self::Request(message) => Message::ConnectionFailed {
                message: message.clone(),
            },
            Self::Api { status, message } => match *status {
                401 | 403 => Message::InvalidApiKey,
                429 => Message::TooManyRequests,
                456 => Message::QuotaExceeded,
                status => Message::ServerError {
                    status,
                    message: message.clone(),
                },
            },
            Self::InvalidResponse => Message::UnreadableResponse,
        }
    }
}

impl fmt::Display for TranslateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message().text())
    }
}

impl std::error::Error for TranslateError {}

#[derive(Serialize)]
struct TranslateRequest<'a> {
    text: [&'a str; 1],
    target_lang: &'a str,
}

#[derive(Deserialize)]
struct TranslateResponse {
    translations: Vec<TranslationPayload>,
}

#[derive(Deserialize)]
struct TranslationPayload {
    text: String,
    detected_source_language: Option<String>,
}

#[derive(Deserialize)]
struct LanguagePayload {
    lang: String,
    name: String,
    usable_as_target: bool,
}

#[derive(Deserialize)]
struct ErrorPayload {
    message: Option<String>,
}

pub fn translate(
    text: &str,
    target_language: &str,
    auth_key: &str,
    use_free_api: bool,
) -> Result<Translation, TranslateError> {
    if text.trim().is_empty() {
        return Err(TranslateError::EmptyText);
    }
    if text.len() > MAX_TEXT_BYTES {
        return Err(TranslateError::TextTooLarge);
    }

    let request = client()?
        .post(endpoint(use_free_api, TRANSLATE_PATH))
        .timeout(TRANSLATE_TIMEOUT)
        .json(&TranslateRequest {
            text: [text],
            target_lang: target_language,
        });
    let body = send(request, auth_key)?;

    let payload: TranslateResponse =
        serde_json::from_str(&body).map_err(|_| TranslateError::InvalidResponse)?;
    let translation = payload
        .translations
        .into_iter()
        .next()
        .ok_or(TranslateError::InvalidResponse)?;

    Ok(Translation {
        text: translation.text,
        detected_source_language: translation.detected_source_language,
    })
}

pub fn target_languages(
    auth_key: &str,
    use_free_api: bool,
) -> Result<Vec<TargetLanguage>, TranslateError> {
    let request = client()?
        .get(endpoint(use_free_api, LANGUAGES_PATH))
        .timeout(LANGUAGES_TIMEOUT);
    parse_target_languages(&send(request, auth_key)?)
}

/// One client for the whole app, so requests reuse its connections instead of
/// setting up TLS again for every translation.
fn client() -> Result<&'static Client, TranslateError> {
    static CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            Client::builder()
                .connect_timeout(Duration::from_secs(8))
                .user_agent(concat!("DeepL-Popup/", env!("CARGO_PKG_VERSION")))
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|error| TranslateError::Request(error.clone()))
}

fn endpoint(use_free_api: bool, path: &str) -> String {
    let base = if use_free_api {
        FREE_API_BASE
    } else {
        PRO_API_BASE
    };
    format!("{base}{path}")
}

/// Sends an authenticated request and returns the body of a successful response.
fn send(request: RequestBuilder, auth_key: &str) -> Result<String, TranslateError> {
    let response = request
        .header(AUTHORIZATION, format!("DeepL-Auth-Key {}", auth_key.trim()))
        .send()
        .map_err(|error| TranslateError::Request(error.to_string()))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|error| TranslateError::Request(error.to_string()))?;

    if status.is_success() {
        return Ok(body);
    }
    let message = serde_json::from_str::<ErrorPayload>(&body)
        .ok()
        .and_then(|payload| payload.message)
        .unwrap_or_default();
    Err(TranslateError::Api {
        status: status.as_u16(),
        message: truncate_message(&message),
    })
}
fn parse_target_languages(body: &str) -> Result<Vec<TargetLanguage>, TranslateError> {
    let payload: Vec<LanguagePayload> =
        serde_json::from_str(body).map_err(|_| TranslateError::InvalidResponse)?;
    let languages = payload
        .into_iter()
        .filter(|language| language.usable_as_target)
        .filter_map(|language| {
            let code = language.lang.trim();
            let name = language.name.trim();
            (!code.is_empty() && !name.is_empty()).then(|| TargetLanguage::new(code, name))
        })
        .collect::<Vec<_>>();

    if languages.is_empty() {
        Err(TranslateError::InvalidResponse)
    } else {
        Ok(languages)
    }
}

fn truncate_message(message: &str) -> String {
    message.chars().take(240).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_translation_response() {
        let response: TranslateResponse = serde_json::from_str(
            r#"{"translations":[{"detected_source_language":"EN","text":"Hello"}]}"#,
        )
        .unwrap();

        assert_eq!(response.translations[0].text, "Hello");
        assert_eq!(
            response.translations[0].detected_source_language.as_deref(),
            Some("EN")
        );
    }

    #[test]
    fn limits_server_error_messages() {
        assert_eq!(truncate_message(&"x".repeat(300)).len(), 240);
    }

    #[test]
    fn parses_only_usable_target_languages() {
        let languages = parse_target_languages(
            r#"[
                {"lang":"en","name":"English","usable_as_target":false},
                {"lang":"en-US","name":"English (American)","usable_as_target":true},
                {"lang":"de-CH","name":"German (Swiss)","usable_as_target":true}
            ]"#,
        )
        .unwrap();

        assert_eq!(
            languages,
            vec![
                TargetLanguage::new("en-US", "English (American)"),
                TargetLanguage::new("de-CH", "German (Swiss)"),
            ]
        );
    }
}

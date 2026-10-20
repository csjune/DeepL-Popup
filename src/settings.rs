use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::i18n::InterfaceLanguage;
use crate::languages::TargetLanguage;
use crate::messages::Message;

const APP_DIRECTORY: &str = "DeepL Popup";
const SETTINGS_FILE: &str = "settings.json";
const DEFAULT_SUGGESTED_LANGUAGES: [&str; 2] = ["KO", "EN-US"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub use_free_api: bool,
    /// Recently chosen target languages, most recent first. The first entry is the target.
    pub suggested_languages: Vec<String>,
    /// The last language list received from DeepL, so it is complete at startup.
    pub target_languages: Vec<TargetLanguage>,
    pub interface_language: InterfaceLanguage,
    // Fields written by earlier versions, read once to migrate to `suggested_languages`.
    #[serde(skip_serializing)]
    auto_target: Option<bool>,
    #[serde(skip_serializing)]
    target_language_code: Option<String>,
    #[serde(skip_serializing)]
    preferred_language_code: Option<String>,
    #[serde(skip_serializing)]
    secondary_language_code: Option<String>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self::new(true, Vec::new(), Vec::new())
    }
}

impl Preferences {
    pub fn new(
        use_free_api: bool,
        suggested_languages: Vec<String>,
        target_languages: Vec<TargetLanguage>,
    ) -> Self {
        Self {
            use_free_api,
            suggested_languages,
            target_languages,
            interface_language: InterfaceLanguage::default(),
            auto_target: None,
            target_language_code: None,
            preferred_language_code: None,
            secondary_language_code: None,
        }
    }

    pub fn load() -> Self {
        let mut preferences = fs::read_to_string(settings_path())
            .ok()
            .and_then(|contents| serde_json::from_str::<Self>(&contents).ok())
            .unwrap_or_default();
        preferences.migrate_suggested_languages();
        preferences
    }

    fn migrate_suggested_languages(&mut self) {
        let mut languages = std::mem::take(&mut self.suggested_languages);
        if languages.is_empty() {
            if self.auto_target == Some(false) {
                languages.extend(self.target_language_code.clone());
            }
            languages.extend(self.preferred_language_code.clone());
            languages.extend(self.secondary_language_code.clone());
        }
        if languages.is_empty() {
            languages.extend(DEFAULT_SUGGESTED_LANGUAGES.map(str::to_owned));
        }

        for language in languages {
            let language = language.trim();
            if !language.is_empty()
                && !self
                    .suggested_languages
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(language))
            {
                self.suggested_languages.push(language.to_owned());
            }
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = settings_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                Message::SettingsFolderFailed {
                    error: error.to_string(),
                }
                .text()
            })?;
        }
        let contents = serde_json::to_vec_pretty(self).map_err(|error| {
            Message::SettingsSerializeFailed {
                error: error.to_string(),
            }
            .text()
        })?;
        fs::write(path, contents).map_err(|error| {
            Message::SettingsSaveFailed {
                error: error.to_string(),
            }
            .text()
        })
    }
}

pub fn load_api_key() -> Result<Option<String>, String> {
    if let Ok(value) = env::var("DEEPL_AUTH_KEY") {
        let value = value.trim().to_owned();
        if !value.is_empty() {
            return Ok(Some(value));
        }
    }

    credential_store::load()
}

pub fn save_api_key(api_key: &str) -> Result<(), String> {
    credential_store::save(api_key.trim())
}

fn settings_path() -> PathBuf {
    if let Some(app_data) = env::var_os("APPDATA") {
        PathBuf::from(app_data)
            .join(APP_DIRECTORY)
            .join(SETTINGS_FILE)
    } else {
        PathBuf::from(SETTINGS_FILE)
    }
}

#[cfg(windows)]
mod credential_store {
    use std::ptr;
    use std::slice;

    use windows::Win32::Foundation::ERROR_NOT_FOUND;
    use windows::Win32::Security::Credentials::{
        CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree,
        CredReadW, CredWriteW,
    };
    use windows::core::{PCWSTR, PWSTR};

    use crate::messages::Message;

    const TARGET_NAME: &str = "DeepL Popup/API Auth Key";
    const USER_NAME: &str = "DeepL Popup";

    pub fn load() -> Result<Option<String>, String> {
        let target_name = wide_null(TARGET_NAME);
        let mut credential = ptr::null_mut::<CREDENTIALW>();
        let read_result = unsafe {
            CredReadW(
                PCWSTR(target_name.as_ptr()),
                CRED_TYPE_GENERIC,
                None,
                &mut credential,
            )
        };

        if let Err(error) = read_result {
            if error.code() == ERROR_NOT_FOUND.to_hresult() {
                return Ok(None);
            }
            return Err(Message::ApiKeyReadFailed {
                error: error.to_string(),
            }
            .text());
        }
        if credential.is_null() {
            return Err(Message::InvalidCredential.text());
        }

        let result = unsafe {
            (|| {
                let credential_ref = &*credential;
                if credential_ref.CredentialBlobSize == 0 {
                    return Ok(None);
                }
                if credential_ref.CredentialBlob.is_null() {
                    return Err(Message::StoredApiKeyInvalid.text());
                }
                let bytes = slice::from_raw_parts(
                    credential_ref.CredentialBlob,
                    credential_ref.CredentialBlobSize as usize,
                );
                String::from_utf8(bytes.to_vec())
                    .map(Some)
                    .map_err(|_| Message::StoredApiKeyInvalid.text())
            })()
        };

        unsafe { CredFree(credential.cast()) };
        result
    }

    pub fn save(api_key: &str) -> Result<(), String> {
        let mut target_name = wide_null(TARGET_NAME);

        if api_key.is_empty() {
            let _ = unsafe { CredDeleteW(PCWSTR(target_name.as_ptr()), CRED_TYPE_GENERIC, None) };
            return Ok(());
        }

        let mut user_name = wide_null(USER_NAME);
        let mut blob = api_key.as_bytes().to_vec();
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target_name.as_mut_ptr()),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: PWSTR(user_name.as_mut_ptr()),
            ..Default::default()
        };

        unsafe { CredWriteW(&credential, 0) }.map_err(|error| {
            Message::ApiKeyStoreFailed {
                error: error.to_string(),
            }
            .text()
        })
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
}

#[cfg(not(windows))]
mod credential_store {
    use crate::messages::Message;

    pub fn load() -> Result<Option<String>, String> {
        Ok(None)
    }

    pub fn save(_api_key: &str) -> Result<(), String> {
        Err(Message::ApiKeyStorageUnsupported.text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Preferences {
        let mut preferences: Preferences = serde_json::from_str(json).unwrap();
        preferences.migrate_suggested_languages();
        preferences
    }

    #[test]
    fn missing_fields_use_defaults() {
        let preferences = parse("{}");
        assert!(preferences.use_free_api);
        assert_eq!(preferences.suggested_languages, ["KO", "EN-US"]);
    }

    #[test]
    fn legacy_language_settings_become_suggested_languages() {
        let preferences = parse(
            r#"{"auto_target":false,"target_language_code":"JA","preferred_language_code":"KO","secondary_language_code":"ja"}"#,
        );
        assert_eq!(preferences.suggested_languages, ["JA", "KO"]);

        let saved = serde_json::to_string(&preferences).unwrap();
        assert!(!saved.contains("preferred_language_code"));
    }
}

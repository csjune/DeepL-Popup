use serde::{Deserialize, Serialize};

use crate::i18n::{self, Language};

/// Target languages shown before the DeepL language list has been fetched.
const FALLBACK_LANGUAGES: [(&str, &str); 20] = [
    ("KO", "Korean"),
    ("EN-US", "English (American)"),
    ("EN-GB", "English (British)"),
    ("JA", "Japanese"),
    ("ZH-HANS", "Chinese (Simplified)"),
    ("ZH-HANT", "Chinese (Traditional)"),
    ("DE", "German"),
    ("FR", "French"),
    ("ES", "Spanish"),
    ("IT", "Italian"),
    ("PT-BR", "Portuguese (Brazilian)"),
    ("PT-PT", "Portuguese (European)"),
    ("NL", "Dutch"),
    ("PL", "Polish"),
    ("RU", "Russian"),
    ("UK", "Ukrainian"),
    ("TR", "Turkish"),
    ("SV", "Swedish"),
    ("DA", "Danish"),
    ("ID", "Indonesian"),
];

const KOREAN_NAMES: &[(&str, &str)] = &[
    ("ace", "아체어"),
    ("af", "아프리칸스어"),
    ("an", "아라곤어"),
    ("ar", "아랍어"),
    ("as", "아삼어"),
    ("ay", "아이마라어"),
    ("az", "아제르바이잔어"),
    ("ba", "바시키르어"),
    ("be", "벨라루스어"),
    ("bg", "불가리아어"),
    ("bho", "보지푸리어"),
    ("bn", "벵골어"),
    ("br", "브르타뉴어"),
    ("bs", "보스니아어"),
    ("ca", "카탈루냐어"),
    ("ceb", "세부아노어"),
    ("ckb", "쿠르드어(소라니)"),
    ("cs", "체코어"),
    ("cy", "웨일스어"),
    ("da", "덴마크어"),
    ("de", "독일어"),
    ("de-ch", "독일어(스위스)"),
    ("de-de", "독일어"),
    ("el", "그리스어"),
    ("en", "영어"),
    ("en-gb", "영어(영국)"),
    ("en-us", "영어(미국)"),
    ("eo", "에스페란토어"),
    ("es", "스페인어"),
    ("es-419", "스페인어(라틴 아메리카)"),
    ("es-es", "스페인어"),
    ("et", "에스토니아어"),
    ("eu", "바스크어"),
    ("fa", "페르시아어"),
    ("fi", "핀란드어"),
    ("fr", "프랑스어"),
    ("fr-ca", "프랑스어(캐나다)"),
    ("fr-fr", "프랑스어"),
    ("ga", "아일랜드어"),
    ("gl", "갈리시아어"),
    ("gn", "과라니어"),
    ("gom", "콘칸어"),
    ("gu", "구자라트어"),
    ("ha", "하우사어"),
    ("he", "히브리어"),
    ("hi", "힌디어"),
    ("hr", "크로아티아어"),
    ("ht", "아이티 크리올어"),
    ("hu", "헝가리어"),
    ("hy", "아르메니아어"),
    ("id", "인도네시아어"),
    ("ig", "이그보어"),
    ("is", "아이슬란드어"),
    ("it", "이탈리아어"),
    ("ja", "일본어"),
    ("jv", "자바어"),
    ("ka", "조지아어"),
    ("kk", "카자흐어"),
    ("kmr", "쿠르드어(쿠르만지)"),
    ("ko", "한국어"),
    ("ky", "키르기스어"),
    ("la", "라틴어"),
    ("lb", "룩셈부르크어"),
    ("lmo", "롬바르드어"),
    ("ln", "링갈라어"),
    ("lt", "리투아니아어"),
    ("lv", "라트비아어"),
    ("mai", "마이틸리어"),
    ("mg", "말라가시어"),
    ("mi", "마오리어"),
    ("mk", "마케도니아어"),
    ("ml", "말라얄람어"),
    ("mn", "몽골어"),
    ("mr", "마라티어"),
    ("ms", "말레이어"),
    ("mt", "몰타어"),
    ("my", "버마어"),
    ("nb", "노르웨이어(보크몰)"),
    ("ne", "네팔어"),
    ("nl", "네덜란드어"),
    ("oc", "오크어"),
    ("om", "오로모어"),
    ("pa", "펀자브어"),
    ("pag", "팡가시난어"),
    ("pam", "팜팡가어"),
    ("pl", "폴란드어"),
    ("prs", "다리어"),
    ("ps", "파슈토어"),
    ("pt", "포르투갈어"),
    ("pt-br", "포르투갈어(브라질)"),
    ("pt-pt", "포르투갈어(포르투갈)"),
    ("qu", "케추아어"),
    ("ro", "루마니아어"),
    ("ru", "러시아어"),
    ("sa", "산스크리트어"),
    ("scn", "시칠리아어"),
    ("sk", "슬로바키아어"),
    ("sl", "슬로베니아어"),
    ("sq", "알바니아어"),
    ("sr", "세르비아어"),
    ("st", "소토어"),
    ("su", "순다어"),
    ("sv", "스웨덴어"),
    ("sw", "스와힐리어"),
    ("ta", "타밀어"),
    ("te", "텔루구어"),
    ("tg", "타지크어"),
    ("th", "태국어"),
    ("tk", "투르크멘어"),
    ("tl", "타갈로그어"),
    ("tn", "츠와나어"),
    ("tr", "튀르키예어"),
    ("ts", "총가어"),
    ("tt", "타타르어"),
    ("uk", "우크라이나어"),
    ("ur", "우르두어"),
    ("uz", "우즈베크어"),
    ("vi", "베트남어"),
    ("wo", "월로프어"),
    ("xh", "코사어"),
    ("yi", "이디시어"),
    ("yue", "광둥어"),
    ("zh", "중국어"),
    ("zh-hans", "중국어(간체)"),
    ("zh-hant", "중국어(번체)"),
    ("zu", "줄루어"),
];

/// A language DeepL can translate into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "StoredLanguage", into = "StoredLanguage")]
pub struct TargetLanguage {
    pub code: String,
    /// The name DeepL reports, in English.
    pub name: String,
    /// The name shown in the UI, in the interface language: DeepL's English
    /// name, or the Korean name when the interface is Korean and it is known.
    pub display_name: String,
}

impl TargetLanguage {
    pub fn new(code: impl Into<String>, name: impl Into<String>) -> Self {
        Self::localized(code, name, i18n::current())
    }

    pub fn localized(code: impl Into<String>, name: impl Into<String>, language: Language) -> Self {
        let code = code.into();
        let name = name.into();
        let korean = match language {
            Language::Korean => KOREAN_NAMES
                .iter()
                .find(|(known, _)| known.eq_ignore_ascii_case(&code)),
            Language::English => None,
        };
        let display_name = korean.map_or_else(|| name.clone(), |(_, korean)| (*korean).to_owned());
        Self {
            code,
            name,
            display_name,
        }
    }
}

/// How a language is saved with the preferences; the display name is derived.
#[derive(Serialize, Deserialize)]
struct StoredLanguage {
    code: String,
    name: String,
}

impl From<StoredLanguage> for TargetLanguage {
    fn from(stored: StoredLanguage) -> Self {
        Self::new(stored.code, stored.name)
    }
}

impl From<TargetLanguage> for StoredLanguage {
    fn from(language: TargetLanguage) -> Self {
        Self {
            code: language.code,
            name: language.name,
        }
    }
}

pub fn fallback_languages() -> Vec<TargetLanguage> {
    prepare(
        FALLBACK_LANGUAGES
            .into_iter()
            .map(|(code, name)| TargetLanguage::new(code, name))
            .collect(),
    )
}

/// The same languages named and sorted for `language`.
pub fn relocalize(languages: &[TargetLanguage], language: Language) -> Vec<TargetLanguage> {
    prepare(
        languages
            .iter()
            .map(|existing| TargetLanguage::localized(&existing.code, &existing.name, language))
            .collect(),
    )
}

/// Removes duplicates and sorts by display name, the order of "all languages".
pub fn prepare(languages: Vec<TargetLanguage>) -> Vec<TargetLanguage> {
    let mut languages = dedupe(languages);
    languages.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    languages
}

/// DeepL lists some languages twice, e.g. `de` and `de-DE`. Keeps the shortest
/// code of each displayed name so the list has no duplicates.
fn dedupe(languages: Vec<TargetLanguage>) -> Vec<TargetLanguage> {
    let mut kept: Vec<TargetLanguage> = Vec::with_capacity(languages.len());
    for language in languages {
        match kept
            .iter_mut()
            .find(|existing| existing.display_name == language.display_name)
        {
            Some(existing) if language.code.len() < existing.code.len() => *existing = language,
            Some(_) => {}
            None => kept.push(language),
        }
    }
    kept
}

/// The base language of a code, e.g. `en` for `EN-US`.
pub fn family(code: &str) -> String {
    code.split('-').next().unwrap_or(code).to_ascii_lowercase()
}

pub fn same_family(left: &str, right: &str) -> bool {
    let left = family(left);
    !left.is_empty() && left == family(right)
}

pub fn find<'a>(languages: &'a [TargetLanguage], code: &str) -> Option<&'a TargetLanguage> {
    languages
        .iter()
        .find(|language| language.code.eq_ignore_ascii_case(code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regional_codes_share_a_family() {
        assert!(same_family("EN", "en-US"));
        assert!(same_family("PT-BR", "pt-PT"));
        assert!(same_family("ZH", "zh-Hans"));
        assert!(!same_family("KO", "JA"));
        assert!(!same_family("", ""));
    }

    #[test]
    fn prepared_list_has_no_duplicates_and_is_sorted() {
        let languages = prepare(vec![
            TargetLanguage::new("ko", "Korean"),
            TargetLanguage::new("de-DE", "German"),
            TargetLanguage::new("de", "German"),
            TargetLanguage::new("de-CH", "German (Swiss)"),
        ]);
        let codes = languages
            .iter()
            .map(|language| language.code.as_str())
            .collect::<Vec<_>>();
        assert_eq!(codes, ["de", "de-CH", "ko"]);
    }

    #[test]
    fn korean_interface_uses_korean_names_when_known() {
        let korean =
            |code, name| TargetLanguage::localized(code, name, Language::Korean).display_name;
        assert_eq!(korean("en-US", "English (American)"), "영어(미국)");
        assert_eq!(korean("xx", "Unknown"), "Unknown");
    }

    #[test]
    fn english_interface_uses_deepl_names() {
        let language = TargetLanguage::localized("en-US", "English (American)", Language::English);
        assert_eq!(language.display_name, "English (American)");
    }

    #[test]
    fn relocalized_lists_are_renamed_and_resorted() {
        let english = prepare(vec![
            TargetLanguage::localized("ko", "Korean", Language::English),
            TargetLanguage::localized("de", "German", Language::English),
        ]);
        let korean = relocalize(&english, Language::Korean);
        let names = korean
            .iter()
            .map(|language| language.display_name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["독일어", "한국어"]);
    }

    #[test]
    fn stored_languages_keep_only_the_deepl_name() {
        let language = TargetLanguage::localized("ja", "Japanese", Language::Korean);
        let json = serde_json::to_string(&language).unwrap();
        assert_eq!(json, r#"{"code":"ja","name":"Japanese"}"#);
    }
}

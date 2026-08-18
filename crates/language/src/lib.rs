mod error;
pub use error::*;

#[cfg(feature = "detect")]
mod detect;
#[cfg(feature = "detect")]
pub use detect::detect;

#[cfg(feature = "whisper")]
mod whisper;

use std::str::FromStr;

pub use codes_iso_639::part_1::LanguageCode as ISO639;

pub const PARAKEET_TDT_V3_LANGUAGE_CODES: &[&str] = &[
    "bg", "cs", "da", "de", "el", "en", "es", "et", "fi", "fr", "hr", "hu", "it", "lt", "lv", "mt",
    "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "uk",
];

#[derive(Debug, Clone, PartialEq, schemars::JsonSchema)]
pub struct Language {
    #[schemars(
        with = "String",
        regex(
            pattern = "^[a-zA-Z]{2}(-[a-zA-Z]{4})?(-([a-zA-Z]{2}|[0-9]{3}))?(-[a-zA-Z0-9]{4,8})*$"
        )
    )]
    iso639: ISO639,
    #[schemars(skip)]
    script: Option<String>,
    #[schemars(skip)]
    region: Option<String>,
    #[schemars(skip)]
    variants: Vec<String>,
}

impl Language {
    pub fn new(iso639: ISO639) -> Self {
        Self {
            iso639,
            script: None,
            region: None,
            variants: Vec::new(),
        }
    }

    pub fn with_region(iso639: ISO639, region: impl Into<String>) -> Self {
        Self {
            iso639,
            script: None,
            region: Some(region.into()),
            variants: Vec::new(),
        }
    }

    pub fn iso639(&self) -> ISO639 {
        self.iso639
    }

    pub fn iso639_code(&self) -> &str {
        self.iso639.code()
    }

    pub fn script(&self) -> Option<&str> {
        self.script.as_deref()
    }

    pub fn region(&self) -> Option<&str> {
        self.region.as_deref()
    }

    pub fn variants(&self) -> &[String] {
        &self.variants
    }

    pub fn bcp47_code(&self) -> String {
        let mut code = self.iso639.code().to_string();
        if let Some(script) = &self.script {
            code.push('-');
            code.push_str(script);
        }
        if let Some(region) = &self.region {
            code.push('-');
            code.push_str(region);
        }
        for variant in &self.variants {
            code.push('-');
            code.push_str(variant);
        }
        code
    }

    // Fallbacks derive less specific codes instead of mutating the canonical
    // tag. An alpha region intentionally blocks the language-only fallback
    // ("en-US" does not match ["en"]), matching the pre-script-aware behavior
    // where only region-less tags fell back to the bare language code.
    pub fn matches_any_code(&self, supported: &[&str]) -> bool {
        let bcp47 = self.bcp47_code();
        if supported.contains(&bcp47.as_str()) {
            return true;
        }
        match &self.region {
            Some(region) if region.chars().all(|c| c.is_ascii_alphabetic()) => {
                let language_region = format!("{}-{}", self.iso639.code(), region);
                language_region != bcp47 && supported.contains(&language_region.as_str())
            }
            _ => supported.contains(&self.iso639.code()),
        }
    }
}

fn is_ascii_alpha(part: &str) -> bool {
    part.chars().all(|c| c.is_ascii_alphabetic())
}

fn titlecase(part: &str) -> String {
    let lower = part.to_lowercase();
    let mut chars = lower.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => lower,
    }
}

impl specta::Type for Language {
    fn inline(_: &mut specta::TypeCollection, _: specta::Generics) -> specta::DataType {
        specta::DataType::Primitive(specta::datatype::PrimitiveType::String)
    }
}

impl Default for Language {
    fn default() -> Self {
        Self::new(ISO639::En)
    }
}

impl From<ISO639> for Language {
    fn from(language: ISO639) -> Self {
        Self::new(language)
    }
}

impl std::ops::Deref for Language {
    type Target = ISO639;

    fn deref(&self) -> &Self::Target {
        &self.iso639
    }
}

impl FromStr for Language {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(['-', '_']).collect();

        if parts.is_empty() {
            return Err(Error::InvalidLanguageCode(s.to_string()));
        }

        let lang_part = parts[0].to_lowercase();
        let iso639 =
            ISO639::from_str(&lang_part).map_err(|_| Error::InvalidLanguageCode(s.to_string()))?;

        let mut script = None;
        let mut region = None;
        let mut variants = Vec::new();

        for part in &parts[1..] {
            // A singleton starts extension/private-use subtags, which are not
            // retained.
            if part.len() == 1 {
                break;
            }

            let script_candidate = part.len() == 4 && is_ascii_alpha(part);
            let region_candidate = (part.len() == 2 && is_ascii_alpha(part))
                || (part.len() == 3 && part.chars().all(|c| c.is_ascii_digit()));
            let variant_candidate = part.chars().all(|c| c.is_ascii_alphanumeric())
                && ((5..=8).contains(&part.len())
                    || (part.len() == 4
                        && part.chars().next().is_some_and(|c| c.is_ascii_digit())));

            if script.is_none() && region.is_none() && variants.is_empty() && script_candidate {
                script = Some(titlecase(part));
            } else if region.is_none() && variants.is_empty() && region_candidate {
                region = Some(part.to_uppercase());
            } else if variant_candidate {
                variants.push(part.to_lowercase());
            }
        }

        Ok(Self {
            iso639,
            script,
            region,
            variants,
        })
    }
}

impl serde::Serialize for Language {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.bcp47_code())
    }
}

impl<'de> serde::Deserialize<'de> for Language {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let code = String::deserialize(deserializer)?;
        code.parse().map_err(serde::de::Error::custom)
    }
}

pub fn whisper_multilingual() -> Vec<Language> {
    [
        ISO639::Af,
        ISO639::Am,
        ISO639::Ar,
        ISO639::As,
        ISO639::Az,
        ISO639::Ba,
        ISO639::Be,
        ISO639::Bg,
        ISO639::Bn,
        ISO639::Bo,
        ISO639::Br,
        ISO639::Bs,
        ISO639::Ca,
        ISO639::Cs,
        ISO639::Cy,
        ISO639::Da,
        ISO639::De,
        ISO639::El,
        ISO639::En,
        ISO639::Es,
        ISO639::Et,
        ISO639::Eu,
        ISO639::Fa,
        ISO639::Fi,
        ISO639::Fo,
        ISO639::Fr,
        ISO639::Gl,
        ISO639::Gu,
        ISO639::Ha,
        ISO639::He,
        ISO639::Hi,
        ISO639::Hr,
        ISO639::Ht,
        ISO639::Hu,
        ISO639::Hy,
        ISO639::Id,
        ISO639::Is,
        ISO639::It,
        ISO639::Ja,
        ISO639::Jv,
        ISO639::Ka,
        ISO639::Kk,
        ISO639::Km,
        ISO639::Kn,
        ISO639::Ko,
        ISO639::La,
        ISO639::Lb,
        ISO639::Lo,
        ISO639::Lt,
        ISO639::Lv,
        ISO639::Mg,
        ISO639::Mi,
        ISO639::Mk,
        ISO639::Ml,
        ISO639::Mn,
        ISO639::Mr,
        ISO639::Ms,
        ISO639::Mt,
        ISO639::My,
        ISO639::Ne,
        ISO639::Nl,
        ISO639::Nn,
        ISO639::No,
        ISO639::Oc,
        ISO639::Pa,
        ISO639::Pl,
        ISO639::Ps,
        ISO639::Pt,
        ISO639::Ro,
        ISO639::Ru,
        ISO639::Sa,
        ISO639::Sd,
        ISO639::Si,
        ISO639::Sk,
        ISO639::Sl,
        ISO639::Sn,
        ISO639::So,
        ISO639::Sq,
        ISO639::Sr,
        ISO639::Su,
        ISO639::Sv,
        ISO639::Sw,
        ISO639::Ta,
        ISO639::Te,
        ISO639::Tg,
        ISO639::Th,
        ISO639::Tk,
        ISO639::Tl,
        ISO639::Tr,
        ISO639::Tt,
        ISO639::Uk,
        ISO639::Ur,
        ISO639::Uz,
        ISO639::Vi,
        ISO639::Yi,
        ISO639::Yo,
        ISO639::Zh,
    ]
    .into_iter()
    .map(Language::from)
    .collect()
}

pub fn parakeet_tdt_v3_languages() -> Vec<Language> {
    PARAKEET_TDT_V3_LANGUAGE_CODES
        .iter()
        .filter_map(|code| code.parse::<ISO639>().ok())
        .map(Language::from)
        .collect()
}

pub fn is_parakeet_tdt_v3_language(language: &Language) -> bool {
    PARAKEET_TDT_V3_LANGUAGE_CODES.contains(&language.iso639_code())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_iso639_only() {
        let lang: Language = "en".parse().unwrap();
        assert_eq!(lang.iso639(), ISO639::En);
        assert_eq!(lang.region(), None);
        assert_eq!(lang.bcp47_code(), "en");
    }

    #[test]
    fn test_parse_with_region() {
        let lang: Language = "en-US".parse().unwrap();
        assert_eq!(lang.iso639(), ISO639::En);
        assert_eq!(lang.region(), Some("US"));
        assert_eq!(lang.bcp47_code(), "en-US");
    }

    #[test]
    fn test_parse_with_underscore() {
        let lang: Language = "ja_JP".parse().unwrap();
        assert_eq!(lang.iso639(), ISO639::Ja);
        assert_eq!(lang.region(), Some("JP"));
        assert_eq!(lang.bcp47_code(), "ja-JP");
    }

    #[test]
    fn test_parse_with_script() {
        let lang: Language = "zh-Hans-CN".parse().unwrap();
        assert_eq!(lang.iso639(), ISO639::Zh);
        assert_eq!(lang.script(), Some("Hans"));
        assert_eq!(lang.region(), Some("CN"));
        assert_eq!(lang.bcp47_code(), "zh-Hans-CN");
    }

    #[test]
    fn test_parse_script_only() {
        let lang: Language = "zh-Hant".parse().unwrap();
        assert_eq!(lang.iso639(), ISO639::Zh);
        assert_eq!(lang.script(), Some("Hant"));
        assert_eq!(lang.region(), None);
        assert_eq!(lang.bcp47_code(), "zh-Hant");
    }

    #[test]
    fn test_parse_numeric_region() {
        let lang: Language = "es-419".parse().unwrap();
        assert_eq!(lang.iso639(), ISO639::Es);
        assert_eq!(lang.script(), None);
        assert_eq!(lang.region(), Some("419"));
        assert_eq!(lang.bcp47_code(), "es-419");
    }

    #[test]
    fn test_parse_normalizes_casing() {
        let lang: Language = "ZH_hans_cn".parse().unwrap();
        assert_eq!(lang.script(), Some("Hans"));
        assert_eq!(lang.region(), Some("CN"));
        assert_eq!(lang.bcp47_code(), "zh-Hans-CN");
    }

    #[test]
    fn test_parse_retains_variants() {
        let lang: Language = "de-DE-1996".parse().unwrap();
        assert_eq!(lang.region(), Some("DE"));
        assert_eq!(lang.variants(), ["1996"]);
        assert_eq!(lang.bcp47_code(), "de-DE-1996");
    }

    #[test]
    fn test_parse_drops_extension_subtags() {
        let lang: Language = "en-US-x-private".parse().unwrap();
        assert_eq!(lang.region(), Some("US"));
        assert!(lang.variants().is_empty());
        assert_eq!(lang.bcp47_code(), "en-US");
    }

    #[test]
    fn test_parse_rejects_unsupported_tags() {
        assert!("".parse::<Language>().is_err());
        assert!("xx".parse::<Language>().is_err());
        assert!("419".parse::<Language>().is_err());
    }

    #[test]
    fn test_serde_round_trips_full_tags() {
        for tag in ["zh-Hans-CN", "zh-Hant", "es-419", "de-DE-1996", "ko-US"] {
            let lang: Language = tag.parse().unwrap();
            let json = serde_json::to_string(&lang).unwrap();
            assert_eq!(json, format!("\"{tag}\""));
            let parsed: Language = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, lang);
        }
    }

    #[test]
    fn test_matches_any_code_fallbacks() {
        let zh_hans_cn: Language = "zh-Hans-CN".parse().unwrap();
        assert!(zh_hans_cn.matches_any_code(&["zh-Hans-CN"]));
        assert!(zh_hans_cn.matches_any_code(&["zh-CN"]));
        assert!(!zh_hans_cn.matches_any_code(&["zh"]));

        let zh_hans: Language = "zh-Hans".parse().unwrap();
        assert!(zh_hans.matches_any_code(&["zh"]));

        let es_419: Language = "es-419".parse().unwrap();
        assert!(es_419.matches_any_code(&["es-419"]));
        assert!(es_419.matches_any_code(&["es"]));

        let en_us: Language = "en-US".parse().unwrap();
        assert!(en_us.matches_any_code(&["en-US"]));
        assert!(!en_us.matches_any_code(&["en"]));

        let en: Language = "en".parse().unwrap();
        assert!(en.matches_any_code(&["en"]));
        assert!(!en.matches_any_code(&["fr", "de"]));
    }

    #[test]
    fn test_parse_korean_us() {
        let lang: Language = "ko-US".parse().unwrap();
        assert_eq!(lang.iso639(), ISO639::Ko);
        assert_eq!(lang.region(), Some("US"));
        assert_eq!(lang.bcp47_code(), "ko-US");
    }

    #[test]
    fn test_serde_roundtrip() {
        let lang: Language = "en-US".parse().unwrap();
        let json = serde_json::to_string(&lang).unwrap();
        assert_eq!(json, "\"en-US\"");

        let parsed: Language = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, lang);
    }

    #[test]
    fn test_serde_iso639_only() {
        let lang: Language = "ko".parse().unwrap();
        let json = serde_json::to_string(&lang).unwrap();
        assert_eq!(json, "\"ko\"");
    }

    #[test]
    fn test_backward_compat_from_iso639() {
        let lang: Language = ISO639::En.into();
        assert_eq!(lang.iso639(), ISO639::En);
        assert_eq!(lang.region(), None);
        assert_eq!(lang.bcp47_code(), "en");
    }

    #[test]
    fn test_parakeet_tdt_v3_language_support() {
        let english_us: Language = "en-US".parse().unwrap();
        let korean: Language = "ko".parse().unwrap();

        assert_eq!(parakeet_tdt_v3_languages().len(), 25);
        assert!(is_parakeet_tdt_v3_language(&english_us));
        assert!(!is_parakeet_tdt_v3_language(&korean));
    }
}

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

#[derive(Debug, Clone, PartialEq, schemars::JsonSchema)]
pub struct Language {
    #[schemars(
        with = "String",
        regex(pattern = "^[a-zA-Z]{2}(-[a-zA-Z]{2,4})?(-[a-zA-Z]{2})?$")
    )]
    iso639: ISO639,
    #[schemars(skip)]
    region: Option<String>,
}

impl Language {
    pub fn new(iso639: ISO639) -> Self {
        Self {
            iso639,
            region: None,
        }
    }

    pub fn with_region(iso639: ISO639, region: impl Into<String>) -> Self {
        Self {
            iso639,
            region: Some(region.into()),
        }
    }

    pub fn iso639(&self) -> ISO639 {
        self.iso639
    }

    pub fn iso639_code(&self) -> &str {
        self.iso639.code()
    }

    pub fn region(&self) -> Option<&str> {
        self.region.as_deref()
    }

    pub fn bcp47_code(&self) -> String {
        match &self.region {
            Some(region) => format!("{}-{}", self.iso639.code(), region),
            None => self.iso639.code().to_string(),
        }
    }

    pub fn matches_any_code(&self, supported: &[&str]) -> bool {
        let bcp47 = self.bcp47_code();
        if supported.contains(&bcp47.as_str()) {
            return true;
        }
        if self.region().is_none() {
            return supported.contains(&self.iso639.code());
        }
        false
    }
}

fn extract_region(parts: &[&str]) -> Option<String> {
    for part in parts.iter().skip(1) {
        if part.len() == 2 && part.chars().all(|c| c.is_ascii_alphabetic()) {
            return Some(part.to_uppercase());
        }
    }
    None
}

impl specta::Type for Language {
    fn inline(_: &mut specta::TypeCollection, _: specta::Generics) -> specta::DataType {
        specta::DataType::Primitive(specta::datatype::PrimitiveType::String)
    }
}

impl Default for Language {
    fn default() -> Self {
        Self {
            iso639: ISO639::En,
            region: None,
        }
    }
}

impl From<ISO639> for Language {
    fn from(language: ISO639) -> Self {
        Self {
            iso639: language,
            region: None,
        }
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

        let region = extract_region(&parts);

        Ok(Self { iso639, region })
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
        assert_eq!(lang.region(), Some("CN"));
        assert_eq!(lang.bcp47_code(), "zh-CN");
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
}

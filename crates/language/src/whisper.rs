use crate::{Error, ISO639, Language};
use anlg_whisper::Language as WL;

// Single source of truth for Whisper language support: both conversion
// directions and `whisper_multilingual()` derive from this table.
// ISO 639-1 `jv` intentionally maps to Whisper's legacy `jw` code.
pub(crate) const WHISPER_LANGUAGES: &[(ISO639, WL)] = &[
    (ISO639::Af, WL::Af),
    (ISO639::Am, WL::Am),
    (ISO639::Ar, WL::Ar),
    (ISO639::As, WL::As),
    (ISO639::Az, WL::Az),
    (ISO639::Ba, WL::Ba),
    (ISO639::Be, WL::Be),
    (ISO639::Bg, WL::Bg),
    (ISO639::Bn, WL::Bn),
    (ISO639::Bo, WL::Bo),
    (ISO639::Br, WL::Br),
    (ISO639::Bs, WL::Bs),
    (ISO639::Ca, WL::Ca),
    (ISO639::Cs, WL::Cs),
    (ISO639::Cy, WL::Cy),
    (ISO639::Da, WL::Da),
    (ISO639::De, WL::De),
    (ISO639::El, WL::El),
    (ISO639::En, WL::En),
    (ISO639::Es, WL::Es),
    (ISO639::Et, WL::Et),
    (ISO639::Eu, WL::Eu),
    (ISO639::Fa, WL::Fa),
    (ISO639::Fi, WL::Fi),
    (ISO639::Fo, WL::Fo),
    (ISO639::Fr, WL::Fr),
    (ISO639::Gl, WL::Gl),
    (ISO639::Gu, WL::Gu),
    (ISO639::Ha, WL::Ha),
    (ISO639::He, WL::He),
    (ISO639::Hi, WL::Hi),
    (ISO639::Hr, WL::Hr),
    (ISO639::Ht, WL::Ht),
    (ISO639::Hu, WL::Hu),
    (ISO639::Hy, WL::Hy),
    (ISO639::Id, WL::Id),
    (ISO639::Is, WL::Is),
    (ISO639::It, WL::It),
    (ISO639::Ja, WL::Ja),
    (ISO639::Jv, WL::Jw),
    (ISO639::Ka, WL::Ka),
    (ISO639::Kk, WL::Kk),
    (ISO639::Km, WL::Km),
    (ISO639::Kn, WL::Kn),
    (ISO639::Ko, WL::Ko),
    (ISO639::La, WL::La),
    (ISO639::Lb, WL::Lb),
    (ISO639::Lo, WL::Lo),
    (ISO639::Lt, WL::Lt),
    (ISO639::Lv, WL::Lv),
    (ISO639::Mg, WL::Mg),
    (ISO639::Mi, WL::Mi),
    (ISO639::Mk, WL::Mk),
    (ISO639::Ml, WL::Ml),
    (ISO639::Mn, WL::Mn),
    (ISO639::Mr, WL::Mr),
    (ISO639::Ms, WL::Ms),
    (ISO639::Mt, WL::Mt),
    (ISO639::My, WL::My),
    (ISO639::Ne, WL::Ne),
    (ISO639::Nl, WL::Nl),
    (ISO639::Nn, WL::Nn),
    (ISO639::No, WL::No),
    (ISO639::Oc, WL::Oc),
    (ISO639::Pa, WL::Pa),
    (ISO639::Pl, WL::Pl),
    (ISO639::Ps, WL::Ps),
    (ISO639::Pt, WL::Pt),
    (ISO639::Ro, WL::Ro),
    (ISO639::Ru, WL::Ru),
    (ISO639::Sa, WL::Sa),
    (ISO639::Sd, WL::Sd),
    (ISO639::Si, WL::Si),
    (ISO639::Sk, WL::Sk),
    (ISO639::Sl, WL::Sl),
    (ISO639::Sn, WL::Sn),
    (ISO639::So, WL::So),
    (ISO639::Sq, WL::Sq),
    (ISO639::Sr, WL::Sr),
    (ISO639::Su, WL::Su),
    (ISO639::Sv, WL::Sv),
    (ISO639::Sw, WL::Sw),
    (ISO639::Ta, WL::Ta),
    (ISO639::Te, WL::Te),
    (ISO639::Tg, WL::Tg),
    (ISO639::Th, WL::Th),
    (ISO639::Tk, WL::Tk),
    (ISO639::Tl, WL::Tl),
    (ISO639::Tr, WL::Tr),
    (ISO639::Tt, WL::Tt),
    (ISO639::Uk, WL::Uk),
    (ISO639::Ur, WL::Ur),
    (ISO639::Uz, WL::Uz),
    (ISO639::Vi, WL::Vi),
    (ISO639::Yi, WL::Yi),
    (ISO639::Yo, WL::Yo),
    (ISO639::Zh, WL::Zh),
];

impl TryInto<WL> for Language {
    type Error = Error;

    fn try_into(self) -> Result<WL, Self::Error> {
        WHISPER_LANGUAGES
            .iter()
            .find(|(iso, _)| *iso == self.iso639)
            .map(|(_, wl)| *wl)
            .ok_or_else(|| Error::NotSupportedLanguage(self.to_string()))
    }
}

impl TryInto<Language> for WL {
    type Error = Error;

    fn try_into(self) -> Result<Language, Self::Error> {
        WHISPER_LANGUAGES
            .iter()
            .find(|(_, wl)| wl.whisper_index() == self.whisper_index())
            .map(|(iso, _)| Language::new(*iso))
            .ok_or_else(|| Error::NotSupportedLanguage(self.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_table_is_bijective() {
        let iso_codes: HashSet<_> = WHISPER_LANGUAGES.iter().map(|(iso, _)| *iso).collect();
        let whisper_codes: HashSet<_> = WHISPER_LANGUAGES
            .iter()
            .map(|(_, wl)| wl.whisper_index())
            .collect();

        assert_eq!(iso_codes.len(), WHISPER_LANGUAGES.len());
        assert_eq!(whisper_codes.len(), WHISPER_LANGUAGES.len());
    }

    #[test]
    fn test_round_trip_all_entries() {
        for (iso, wl) in WHISPER_LANGUAGES {
            let forward: WL = Language::new(*iso).try_into().unwrap();
            assert_eq!(forward.whisper_index(), wl.whisper_index());

            let reverse: Language = (*wl).try_into().unwrap();
            assert_eq!(reverse.iso639(), *iso);
        }
    }

    #[test]
    fn test_javanese_alias() {
        let forward: WL = Language::new(ISO639::Jv).try_into().unwrap();
        assert_eq!(forward.whisper_index(), WL::Jw.whisper_index());

        let reverse: Language = WL::Jw.try_into().unwrap();
        assert_eq!(reverse.iso639(), ISO639::Jv);
    }

    #[test]
    fn test_unsupported_codes() {
        let unsupported_iso: Result<WL, _> = Language::new(ISO639::Ln).try_into();
        assert!(unsupported_iso.is_err());

        for wl in [WL::Haw, WL::Ln, WL::Yue] {
            let unsupported_wl: Result<Language, _> = wl.try_into();
            assert!(unsupported_wl.is_err());
        }
    }

    #[test]
    fn test_multilingual_list_matches_table() {
        let list = crate::whisper_multilingual();
        assert_eq!(list.len(), WHISPER_LANGUAGES.len());
        assert_eq!(list.len(), 97);

        for (language, (iso, _)) in list.iter().zip(WHISPER_LANGUAGES) {
            assert_eq!(language.iso639(), *iso);
            assert_eq!(language.region(), None);
        }
    }
}

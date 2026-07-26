use crate::{ISO639, Language};

pub fn detect(text: &str) -> Language {
    let lang = whichlang::detect_language(text);
    lang.into()
}

impl From<whichlang::Lang> for Language {
    fn from(lang: whichlang::Lang) -> Self {
        use whichlang::Lang;

        let iso639 = match lang {
            Lang::Ara => ISO639::Ar,
            Lang::Cmn => ISO639::Zh,
            Lang::Deu => ISO639::De,
            Lang::Eng => ISO639::En,
            Lang::Fra => ISO639::Fr,
            Lang::Hin => ISO639::Hi,
            Lang::Ita => ISO639::It,
            Lang::Jpn => ISO639::Ja,
            Lang::Kor => ISO639::Ko,
            Lang::Nld => ISO639::Nl,
            Lang::Por => ISO639::Pt,
            Lang::Rus => ISO639::Ru,
            Lang::Spa => ISO639::Es,
            Lang::Swe => ISO639::Sv,
            Lang::Tur => ISO639::Tr,
            Lang::Vie => ISO639::Vi,
        };

        Self::new(iso639)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect() {
        assert_eq!(
            detect("The quick brown fox jumps over the lazy dog.").iso639(),
            ISO639::En
        );
        assert_eq!(
            detect("El rápido zorro marrón salta sobre el perro perezoso.").iso639(),
            ISO639::Es
        );
        assert_eq!(
            detect("Le rapide renard brun saute par-dessus le chien paresseux.").iso639(),
            ISO639::Fr
        );
        assert_eq!(
            detect("Der schnelle braune Fuchs springt über den faulen Hund.").iso639(),
            ISO639::De
        );
    }
}

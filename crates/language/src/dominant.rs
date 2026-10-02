use crate::ISO639;

pub fn dominant_language<'a>(
    texts: impl IntoIterator<Item = &'a str>,
    candidates: &[ISO639],
) -> Option<ISO639> {
    let mut unique_candidates = Vec::new();
    for candidate in candidates {
        if !unique_candidates.contains(candidate) {
            unique_candidates.push(*candidate);
        }
    }
    if unique_candidates.len() < 2 {
        return None;
    }

    let languages = unique_candidates
        .iter()
        .map(|candidate| map_language(*candidate))
        .collect::<Option<Vec<_>>>()?;
    let detector = whatlang::Detector::with_allowlist(languages.clone());
    let mut weights = vec![0_usize; unique_candidates.len()];

    for text in texts {
        let text = text.trim();
        if text.is_empty() {
            continue;
        }

        let weight = text
            .chars()
            .filter(|character| !character.is_whitespace())
            .count();
        if let Some(detected) = detector.detect(text).map(|info| info.lang())
            && let Some(index) = languages.iter().position(|language| *language == detected)
        {
            weights[index] = weights[index].saturating_add(weight);
        }
    }

    let mut winner = None;
    let mut highest_weight = 0;
    for (index, weight) in weights.iter().enumerate() {
        if *weight > highest_weight {
            winner = Some(index);
            highest_weight = *weight;
        }
    }

    winner.map(|index| unique_candidates[index])
}

fn map_language(language: ISO639) -> Option<whatlang::Lang> {
    use whatlang::Lang;

    Some(match language {
        ISO639::Eo => Lang::Epo,
        ISO639::En => Lang::Eng,
        ISO639::Ru => Lang::Rus,
        ISO639::Zh => Lang::Cmn,
        ISO639::Es => Lang::Spa,
        ISO639::Pt => Lang::Por,
        ISO639::It => Lang::Ita,
        ISO639::Bn => Lang::Ben,
        ISO639::Fr => Lang::Fra,
        ISO639::De => Lang::Deu,
        ISO639::Uk => Lang::Ukr,
        ISO639::Ka => Lang::Kat,
        ISO639::Ar => Lang::Ara,
        ISO639::Hi => Lang::Hin,
        ISO639::Ja => Lang::Jpn,
        ISO639::He => Lang::Heb,
        ISO639::Yi => Lang::Yid,
        ISO639::Pl => Lang::Pol,
        ISO639::Am => Lang::Amh,
        ISO639::Jv => Lang::Jav,
        ISO639::Ko => Lang::Kor,
        ISO639::No => Lang::Nob,
        ISO639::Da => Lang::Dan,
        ISO639::Sv => Lang::Swe,
        ISO639::Fi => Lang::Fin,
        ISO639::Tr => Lang::Tur,
        ISO639::Nl => Lang::Nld,
        ISO639::Hu => Lang::Hun,
        ISO639::Cs => Lang::Ces,
        ISO639::El => Lang::Ell,
        ISO639::Bg => Lang::Bul,
        ISO639::Be => Lang::Bel,
        ISO639::Mr => Lang::Mar,
        ISO639::Kn => Lang::Kan,
        ISO639::Ro => Lang::Ron,
        ISO639::Sl => Lang::Slv,
        ISO639::Hr => Lang::Hrv,
        ISO639::Sr => Lang::Srp,
        ISO639::Mk => Lang::Mkd,
        ISO639::Lt => Lang::Lit,
        ISO639::Lv => Lang::Lav,
        ISO639::Et => Lang::Est,
        ISO639::Ta => Lang::Tam,
        ISO639::Vi => Lang::Vie,
        ISO639::Ur => Lang::Urd,
        ISO639::Th => Lang::Tha,
        ISO639::Gu => Lang::Guj,
        ISO639::Uz => Lang::Uzb,
        ISO639::Pa => Lang::Pan,
        ISO639::Az => Lang::Aze,
        ISO639::Id => Lang::Ind,
        ISO639::Te => Lang::Tel,
        ISO639::Fa => Lang::Pes,
        ISO639::Ml => Lang::Mal,
        ISO639::Or => Lang::Ori,
        ISO639::My => Lang::Mya,
        ISO639::Ne => Lang::Nep,
        ISO639::Si => Lang::Sin,
        ISO639::Km => Lang::Khm,
        ISO639::Tk => Lang::Tuk,
        ISO639::Ak => Lang::Aka,
        ISO639::Zu => Lang::Zul,
        ISO639::Sn => Lang::Sna,
        ISO639::Af => Lang::Afr,
        ISO639::La => Lang::Lat,
        ISO639::Sk => Lang::Slk,
        ISO639::Ca => Lang::Cat,
        ISO639::Tl => Lang::Tgl,
        ISO639::Hy => Lang::Hye,
        ISO639::Cy => Lang::Cym,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_english_for_an_english_dominant_mix() {
        assert_eq!(
            dominant_language(
                [
                    "The quick brown fox jumps over the lazy dog.",
                    "안녕하세요 반갑습니다"
                ],
                &[ISO639::En, ISO639::Ko],
            ),
            Some(ISO639::En)
        );
    }

    #[test]
    fn picks_korean_for_a_korean_dominant_mix() {
        assert_eq!(
            dominant_language(
                [
                    "안녕하세요. 오늘 회의에 참석해 주셔서 감사합니다.",
                    "Hi, thanks."
                ],
                &[ISO639::En, ISO639::Ko],
            ),
            Some(ISO639::Ko)
        );
    }

    #[test]
    fn returns_none_for_one_candidate() {
        assert_eq!(dominant_language(["English"], &[ISO639::En]), None);
    }

    #[test]
    fn returns_none_when_a_candidate_cannot_be_mapped() {
        assert_eq!(
            dominant_language(["English"], &[ISO639::En, ISO639::Ab]),
            None
        );
    }

    #[test]
    fn returns_none_for_empty_or_whitespace_texts() {
        assert_eq!(
            dominant_language(["", " \t\n "], &[ISO639::En, ISO639::Ko]),
            None
        );
    }
}

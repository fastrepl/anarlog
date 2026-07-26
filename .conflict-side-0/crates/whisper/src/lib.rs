// https://github.com/openai/whisper/blob/ba3f3cd/whisper/tokenizer.py#L10-L128
#[repr(u8)]
#[derive(Debug, Copy, Clone, strum::EnumString, strum::Display, strum::AsRefStr)]
pub enum Language {
    #[strum(serialize = "en")]
    En,
    #[strum(serialize = "zh")]
    Zh,
    #[strum(serialize = "de")]
    De,
    #[strum(serialize = "es")]
    Es,
    #[strum(serialize = "ru")]
    Ru,
    #[strum(serialize = "ko")]
    Ko,
    #[strum(serialize = "fr")]
    Fr,
    #[strum(serialize = "ja")]
    Ja,
    #[strum(serialize = "pt")]
    Pt,
    #[strum(serialize = "tr")]
    Tr,
    #[strum(serialize = "pl")]
    Pl,
    #[strum(serialize = "ca")]
    Ca,
    #[strum(serialize = "nl")]
    Nl,
    #[strum(serialize = "ar")]
    Ar,
    #[strum(serialize = "sv")]
    Sv,
    #[strum(serialize = "it")]
    It,
    #[strum(serialize = "id")]
    Id,
    #[strum(serialize = "hi")]
    Hi,
    #[strum(serialize = "fi")]
    Fi,
    #[strum(serialize = "vi")]
    Vi,
    #[strum(serialize = "he")]
    He,
    #[strum(serialize = "uk")]
    Uk,
    #[strum(serialize = "el")]
    El,
    #[strum(serialize = "ms")]
    Ms,
    #[strum(serialize = "cs")]
    Cs,
    #[strum(serialize = "ro")]
    Ro,
    #[strum(serialize = "da")]
    Da,
    #[strum(serialize = "hu")]
    Hu,
    #[strum(serialize = "ta")]
    Ta,
    #[strum(serialize = "no")]
    No,
    #[strum(serialize = "th")]
    Th,
    #[strum(serialize = "ur")]
    Ur,
    #[strum(serialize = "hr")]
    Hr,
    #[strum(serialize = "bg")]
    Bg,
    #[strum(serialize = "lt")]
    Lt,
    #[strum(serialize = "la")]
    La,
    #[strum(serialize = "mi")]
    Mi,
    #[strum(serialize = "ml")]
    Ml,
    #[strum(serialize = "cy")]
    Cy,
    #[strum(serialize = "sk")]
    Sk,
    #[strum(serialize = "te")]
    Te,
    #[strum(serialize = "fa")]
    Fa,
    #[strum(serialize = "lv")]
    Lv,
    #[strum(serialize = "bn")]
    Bn,
    #[strum(serialize = "sr")]
    Sr,
    #[strum(serialize = "az")]
    Az,
    #[strum(serialize = "sl")]
    Sl,
    #[strum(serialize = "kn")]
    Kn,
    #[strum(serialize = "et")]
    Et,
    #[strum(serialize = "mk")]
    Mk,
    #[strum(serialize = "br")]
    Br,
    #[strum(serialize = "eu")]
    Eu,
    #[strum(serialize = "is")]
    Is,
    #[strum(serialize = "hy")]
    Hy,
    #[strum(serialize = "ne")]
    Ne,
    #[strum(serialize = "mn")]
    Mn,
    #[strum(serialize = "bs")]
    Bs,
    #[strum(serialize = "kk")]
    Kk,
    #[strum(serialize = "sq")]
    Sq,
    #[strum(serialize = "sw")]
    Sw,
    #[strum(serialize = "gl")]
    Gl,
    #[strum(serialize = "mr")]
    Mr,
    #[strum(serialize = "pa")]
    Pa,
    #[strum(serialize = "si")]
    Si,
    #[strum(serialize = "km")]
    Km,
    #[strum(serialize = "sn")]
    Sn,
    #[strum(serialize = "yo")]
    Yo,
    #[strum(serialize = "so")]
    So,
    #[strum(serialize = "af")]
    Af,
    #[strum(serialize = "oc")]
    Oc,
    #[strum(serialize = "ka")]
    Ka,
    #[strum(serialize = "be")]
    Be,
    #[strum(serialize = "tg")]
    Tg,
    #[strum(serialize = "sd")]
    Sd,
    #[strum(serialize = "gu")]
    Gu,
    #[strum(serialize = "am")]
    Am,
    #[strum(serialize = "yi")]
    Yi,
    #[strum(serialize = "lo")]
    Lo,
    #[strum(serialize = "uz")]
    Uz,
    #[strum(serialize = "fo")]
    Fo,
    #[strum(serialize = "ht")]
    Ht,
    #[strum(serialize = "ps")]
    Ps,
    #[strum(serialize = "tk")]
    Tk,
    #[strum(serialize = "nn")]
    Nn,
    #[strum(serialize = "mt")]
    Mt,
    #[strum(serialize = "sa")]
    Sa,
    #[strum(serialize = "lb")]
    Lb,
    #[strum(serialize = "my")]
    My,
    #[strum(serialize = "bo")]
    Bo,
    #[strum(serialize = "tl")]
    Tl,
    #[strum(serialize = "mg")]
    Mg,
    #[strum(serialize = "as")]
    As,
    #[strum(serialize = "tt")]
    Tt,
    #[strum(serialize = "haw")]
    Haw,
    #[strum(serialize = "ln")]
    Ln,
    #[strum(serialize = "ha")]
    Ha,
    #[strum(serialize = "ba")]
    Ba,
    #[strum(serialize = "jw")]
    Jw,
    #[strum(serialize = "su")]
    Su,
    #[strum(serialize = "yue")]
    Yue,
}

impl Language {
    pub fn whisper_index(self) -> usize {
        self as usize
    }
}

pub const AUDIO: &[u8] = include_wav!("./audio.wav");

pub const AUDIO_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/english_3/audio.wav");

pub const WORDS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/english_3/words.json");
pub const WORDS_JSON: &str = include_str!("./words.json");

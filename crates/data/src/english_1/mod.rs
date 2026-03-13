pub const AUDIO: &[u8] = include_wav!("./audio.wav");
pub const AUDIO_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/english_1/audio.wav");

pub const AUDIO_MP3_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/english_1/audio.mp3");
pub const AUDIO_MP4_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/english_1/audio.mp4");
pub const AUDIO_M4A_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/english_1/audio.m4a");
pub const AUDIO_OGG_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/english_1/audio.ogg");
pub const AUDIO_FLAC_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/english_1/audio.flac");
pub const AUDIO_OPUS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/english_1/audio.opus");
pub const AUDIO_AAC_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/english_1/audio.aac");
pub const AUDIO_AIFF_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/english_1/audio.aiff");
pub const AUDIO_WEBM_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/english_1/audio.webm");
pub const AUDIO_CAF_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/english_1/audio.caf");

pub const AUDIO_PART1_8000HZ_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/english_1/audio_part1_8000hz.wav"
);
pub const AUDIO_PART2_16000HZ_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/english_1/audio_part2_16000hz.wav"
);
pub const AUDIO_PART3_22050HZ_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/english_1/audio_part3_22050hz.wav"
);
pub const AUDIO_PART4_32000HZ_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/english_1/audio_part4_32000hz.wav"
);
pub const AUDIO_PART5_44100HZ_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/english_1/audio_part5_44100hz.wav"
);
pub const AUDIO_PART6_48000HZ_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/english_1/audio_part6_48000hz.wav"
);

pub const TRANSCRIPTION_JSON: &str = include_str!("./transcription.json");

pub const TRANSCRIPTION_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/english_1/transcription.json"
);

pub const DIARIZATION_JSON: &str = include_str!("./diarization.json");

pub const DIARIZATION_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/english_1/diarization.json"
);

pub const DEEPGRAM_JSON: &str = include_str!("./deepgram.json");

pub const SONIOX_JSON: &str = include_str!("./soniox.json");

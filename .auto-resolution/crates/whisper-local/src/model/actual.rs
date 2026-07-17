// https://github.com/tazz4843/whisper-rs/blob/master/examples/audio_transcription.rs

use lazy_static::lazy_static;
use regex::Regex;

use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
    WhisperTokenId,
};

use hypr_whisper::Language;

use crate::Segment;

lazy_static! {
    static ref TRAILING_DOTS: Regex = Regex::new(r"\.{2,}$").unwrap();
}

#[derive(Default)]
pub struct LoadedWhisperBuilder {
    model_path: Option<String>,
}

impl LoadedWhisperBuilder {
    pub fn model_path(mut self, model_path: impl Into<String>) -> Self {
        self.model_path = Some(model_path.into());
        self
    }

    pub fn build(self) -> Result<LoadedWhisper, crate::Error> {
        unsafe { Self::suppress_log() };

        let context_param = {
            let mut p = WhisperContextParameters {
                gpu_device: 0,
                use_gpu: true,
                flash_attn: false, // crash on macos
                ..Default::default()
            };
            p.dtw_parameters.mode = whisper_rs::DtwMode::None;
            p
        };

        let model_path = self.model_path.unwrap();
        if !std::path::Path::new(&model_path).exists() {
            return Err(crate::Error::ModelNotFound);
        }

        let ctx = WhisperContext::new_with_params(&model_path, context_param)?;
        let token_beg = ctx.token_beg();

        Ok(LoadedWhisper { ctx, token_beg })
    }

    unsafe fn suppress_log() {
        unsafe extern "C" fn noop_callback(
            _level: whisper_rs::whisper_rs_sys::ggml_log_level,
            _text: *const ::std::os::raw::c_char,
            _user_data: *mut ::std::os::raw::c_void,
        ) {
        }
        unsafe { whisper_rs::set_log_callback(Some(noop_callback), std::ptr::null_mut()) };
    }
}

#[derive(Default)]
pub struct WhisperBuilder {
    model_path: Option<String>,
    languages: Option<Vec<Language>>,
}

impl WhisperBuilder {
    pub fn model_path(mut self, model_path: impl Into<String>) -> Self {
        self.model_path = Some(model_path.into());
        self
    }

    pub fn languages(mut self, languages: Vec<Language>) -> Self {
        self.languages = Some(languages);
        self
    }

    pub fn build(self) -> Result<Whisper, crate::Error> {
        LoadedWhisper::builder()
            .model_path(self.model_path.unwrap())
            .build()?
            .session(self.languages.unwrap_or_default())
    }
}

pub struct LoadedWhisper {
    ctx: WhisperContext,
    token_beg: WhisperTokenId,
}

impl LoadedWhisper {
    pub fn builder() -> LoadedWhisperBuilder {
        LoadedWhisperBuilder::default()
    }

    pub fn session(&self, languages: Vec<Language>) -> Result<Whisper, crate::Error> {
        Ok(Whisper {
            id: uuid::Uuid::new_v4().to_string(),
            index: 0,
            languages,
            dynamic_prompt: String::new(),
            state: self.ctx.create_state()?,
            token_beg: self.token_beg,
        })
    }
}

pub struct Whisper {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    index: usize,
    languages: Vec<Language>,
    dynamic_prompt: String,
    state: WhisperState,
    token_beg: WhisperTokenId,
}

impl Whisper {
    pub fn builder() -> WhisperBuilder {
        WhisperBuilder::default()
    }

    pub fn transcribe(&mut self, audio: &[f32]) -> Result<Vec<Segment>, crate::Error> {
        #[cfg(debug_assertions)]
        self.debug(audio);

        let input_audio_length_sec = audio.len() as f32 / 16000.0;
        if input_audio_length_sec < 0.1 {
            tracing::warn!(input_audio_length_sec = ?input_audio_length_sec, "transcribe_skipped");
            return Ok(vec![]);
        }

        let token_beg = self.token_beg;
        let language = self.get_language(audio)?;

        let params = {
            let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

            let parts = [self.dynamic_prompt.trim()];
            let joined = parts.join("\n");
            let initial_prompt = joined.trim();

            tracing::info!(input_audio_length_sec = ?input_audio_length_sec, "transcribe_started");

            p.set_translate(false);
            p.set_detect_language(false);
            p.set_language(language.as_deref());

            p.set_initial_prompt(initial_prompt);

            unsafe {
                Self::suppress_beg(&mut p, &token_beg);
            }

            p.set_no_timestamps(true);
            p.set_token_timestamps(false);
            p.set_split_on_word(true);

            p.set_temperature(0.0);
            p.set_temperature_inc(0.2);

            p.set_single_segment(true);
            p.set_suppress_blank(true);
            p.set_suppress_nst(true);

            p.set_print_special(false);
            p.set_print_progress(false);
            p.set_print_realtime(false);
            p.set_print_timestamps(false);
            p
        };

        self.state.full(params, audio)?;
        let num_segments = self.state.full_n_segments();

        let mut segments = Vec::new();
        for i in 0..num_segments {
            let segment = match self.state.get_segment(i) {
                Some(seg) => seg,
                None => continue,
            };

            let (start, end) = (
                (segment.start_timestamp() as f64) / 100.0,
                (segment.end_timestamp() as f64) / 100.0,
            );

            let text = {
                let segment_text = segment.to_str_lossy()?;
                TRAILING_DOTS.replace(&segment_text, "").to_string()
            };

            segments.push(Segment {
                text,
                language: language.clone(),
                start,
                end,
                // https://github.com/ggml-org/whisper.cpp/pull/971/files#diff-2d3599a9fad195f2c3c60bd06691bc1815325b3560b5feda41a91fa71194e805R310-R327
                // We previously implemented it based on above, but after updating to v1.7.6, the API has changed, and we're still unable to figure it out. We're not using it anyway.
                confidence: 1.0,
                ..Default::default()
            });
        }

        let segments = Self::filter_segments(segments);

        let full_text = segments
            .iter()
            .map(|s| s.text())
            .collect::<Vec<&str>>()
            .join(" ");

        if !full_text.is_empty() {
            tracing::info!(text_length = full_text.len(), "transcribe_completed");
            self.dynamic_prompt = full_text;
        }

        Ok(segments)
    }

    fn get_language(&mut self, audio: &[f32]) -> Result<Option<String>, crate::Error> {
        if self.languages.is_empty() {
            tracing::info!("no_language_specified");
            return Ok(None);
        }

        if self.languages.len() == 1 {
            let lang = &self.languages[0];
            tracing::info!("single_language_specified: {}", lang);
            return Ok(Some(lang.to_string()));
        }

        let lang_str = {
            self.state.pcm_to_mel(audio, 1)?;
            let (_lang_id, lang_probs) = self.state.lang_detect(0, 1)?;

            let mut best_lang = None;
            let mut best_prob = f32::NEG_INFINITY;

            for lang in &self.languages {
                let lang_id = lang.whisper_index();
                if lang_id < lang_probs.len() {
                    let prob = lang_probs[lang_id];
                    if prob > best_prob {
                        best_prob = prob;
                        best_lang = Some(lang.as_ref().to_string());
                    }
                }
            }

            tracing::info!("predicted: {:#?}, from: {:#?}", best_lang, self.languages);
            best_lang
        };

        Ok(lang_str)
    }

    fn filter_segments(segments: Vec<Segment>) -> Vec<Segment> {
        segments
            .into_iter()
            .filter(|s| {
                let t = s.text.trim().to_lowercase();

                !(s.confidence < 0.005
                    || t == "you"
                    || t == "thank you"
                    || t == "you."
                    || t == "thank you."
                    || t == "♪")
            })
            .collect()
    }

    unsafe fn suppress_beg(params: &mut FullParams, token_beg: &WhisperTokenId) {
        unsafe extern "C" fn logits_filter_callback(
            _ctx: *mut whisper_rs::whisper_rs_sys::whisper_context,
            _state: *mut whisper_rs::whisper_rs_sys::whisper_state,
            _tokens: *const whisper_rs::whisper_rs_sys::whisper_token_data,
            _n_tokens: std::os::raw::c_int,
            logits: *mut f32,
            user_data: *mut std::os::raw::c_void,
        ) {
            if logits.is_null() || user_data.is_null() {
                return;
            }

            unsafe {
                let token_beg_id = *(user_data as *const WhisperTokenId);
                *logits.offset(token_beg_id as isize) = f32::NEG_INFINITY;
            }
        }

        unsafe {
            params.set_filter_logits_callback(Some(logits_filter_callback));
            params.set_filter_logits_callback_user_data(
                token_beg as *const WhisperTokenId as *mut std::ffi::c_void,
            );
        }
    }

    fn debug(&mut self, audio: &[f32]) {
        if let Ok(v) = std::env::var("HYPR_WHISPER_DEBUG")
            && v == "1"
        {
            let mut writer = hound::WavWriter::create(
                format!("./whisper_{}_{}.wav", self.id, self.index),
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 16000,
                    bits_per_sample: 32,
                    sample_format: hound::SampleFormat::Float,
                },
            )
            .unwrap();
            self.index += 1;

            for sample in audio {
                writer.write_sample(*sample).unwrap();
            }
            writer.finalize().unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whisper() {
        let mut whisper = Whisper::builder()
            .model_path(concat!(env!("CARGO_MANIFEST_DIR"), "/model.bin"))
            .build()
            .unwrap();

        let audio: Vec<f32> = hypr_data::english_1::AUDIO
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
            .collect();

        let start = std::time::Instant::now();
        let segments = whisper.transcribe(&audio).unwrap();
        let duration = start.elapsed();
        println!("segments: {:#?}", segments);
        println!("time: {:?}", duration);
        assert!(segments.len() > 0);
    }
}

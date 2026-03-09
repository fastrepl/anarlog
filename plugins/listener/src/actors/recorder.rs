use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use hypr_audio_utils::{
    decode_vorbis_to_mono_wav_file, decode_vorbis_to_wav_file, mix_audio_f32,
    ogg_has_identical_channels,
};
use ractor::{Actor, ActorName, ActorProcessingErr, ActorRef};
use tauri_plugin_fs_sync::find_session_dir;

const FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1000);

pub enum RecMsg {
    AudioSingle(Arc<[f32]>),
    AudioDual(Arc<[f32]>, Arc<[f32]>),
}

pub struct RecArgs {
    pub app_dir: PathBuf,
    pub session_id: String,
}

pub struct RecState {
    writer: Option<hound::WavWriter<BufWriter<File>>>,
    writer_mic: Option<hound::WavWriter<BufWriter<File>>>,
    writer_spk: Option<hound::WavWriter<BufWriter<File>>>,
    wav_path: PathBuf,
    last_flush: Instant,
    is_stereo: bool,
}

pub struct RecorderActor;

impl RecorderActor {
    pub fn name() -> ActorName {
        "recorder_actor".into()
    }
}

#[ractor::async_trait]
impl Actor for RecorderActor {
    type Msg = RecMsg;
    type State = RecState;
    type Arguments = RecArgs;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let dir = find_session_dir(&args.app_dir, &args.session_id);
        std::fs::create_dir_all(&dir)?;

        let filename_base = "audio".to_string();
        let wav_path = dir.join(format!("{}.wav", filename_base));
        let ogg_path = dir.join(format!("{}.ogg", filename_base));

        let is_stereo = if ogg_path.exists() {
            let has_identical = ogg_has_identical_channels(&ogg_path).map_err(into_actor_err)?;
            if has_identical {
                decode_vorbis_to_mono_wav_file(&ogg_path, &wav_path).map_err(into_actor_err)?;
            } else {
                decode_vorbis_to_wav_file(&ogg_path, &wav_path).map_err(into_actor_err)?;
            }
            std::fs::remove_file(&ogg_path)?;
            !has_identical
        } else if wav_path.exists() {
            let reader = hound::WavReader::open(&wav_path)?;
            reader.spec().channels == 2
        } else {
            true
        };

        let stereo_spec = hound::WavSpec {
            channels: 2,
            sample_rate: super::SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };

        let mono_spec = hound::WavSpec {
            channels: 1,
            sample_rate: super::SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };

        let writer = if wav_path.exists() {
            hound::WavWriter::append(&wav_path)?
        } else if is_stereo {
            hound::WavWriter::create(&wav_path, stereo_spec)?
        } else {
            hound::WavWriter::create(&wav_path, mono_spec)?
        };

        let (writer_mic, writer_spk) = if is_debug_mode() {
            let mic_path = dir.join(format!("{}_mic.wav", filename_base));
            let spk_path = dir.join(format!("{}_spk.wav", filename_base));

            let mic_writer = if mic_path.exists() {
                hound::WavWriter::append(&mic_path)?
            } else {
                hound::WavWriter::create(&mic_path, mono_spec)?
            };

            let spk_writer = if spk_path.exists() {
                hound::WavWriter::append(&spk_path)?
            } else {
                hound::WavWriter::create(&spk_path, mono_spec)?
            };

            (Some(mic_writer), Some(spk_writer))
        } else {
            (None, None)
        };

        Ok(RecState {
            writer: Some(writer),
            writer_mic,
            writer_spk,
            wav_path,
            last_flush: Instant::now(),
            is_stereo,
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        st: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            RecMsg::AudioSingle(samples) => {
                if let Some(ref mut writer) = st.writer {
                    if st.is_stereo {
                        write_mono_as_stereo(writer, &samples)?;
                    } else {
                        write_mono_samples(writer, &samples)?;
                    }
                }
            }
            RecMsg::AudioDual(mic, spk) => {
                if let Some(ref mut writer) = st.writer {
                    if st.is_stereo {
                        write_interleaved_stereo(writer, &mic, &spk)?;
                    } else {
                        let mixed = mix_audio_f32(&mic, &spk);
                        write_mono_samples(writer, &mixed)?;
                    }
                }

                if let Some(ref mut writer_mic) = st.writer_mic {
                    write_mono_samples(writer_mic, &mic)?;
                }

                if let Some(ref mut writer_spk) = st.writer_spk {
                    write_mono_samples(writer_spk, &spk)?;
                }
            }
        }

        flush_if_due(st)?;
        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        st: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        finalize_writer(&mut st.writer, Some(&st.wav_path))?;
        finalize_writer(&mut st.writer_mic, None)?;
        finalize_writer(&mut st.writer_spk, None)?;

        if st.wav_path.exists() {
            sync_file(&st.wav_path);
            sync_dir(&st.wav_path);
        }

        Ok(())
    }
}

fn into_actor_err(err: hypr_audio_utils::Error) -> ActorProcessingErr {
    Box::new(err)
}

fn is_debug_mode() -> bool {
    cfg!(debug_assertions)
        || std::env::var("HYPRNOTE_DEBUG")
            .map(|v| !v.is_empty() && v != "0" && v != "false")
            .unwrap_or(false)
}

fn flush_if_due(state: &mut RecState) -> Result<(), hound::Error> {
    if state.last_flush.elapsed() < FLUSH_INTERVAL {
        return Ok(());
    }
    flush_all(state)
}

fn flush_all(state: &mut RecState) -> Result<(), hound::Error> {
    if let Some(writer) = state.writer.as_mut() {
        writer.flush()?;
    }
    if let Some(writer_mic) = state.writer_mic.as_mut() {
        writer_mic.flush()?;
    }
    if let Some(writer_spk) = state.writer_spk.as_mut() {
        writer_spk.flush()?;
    }
    state.last_flush = Instant::now();
    Ok(())
}

fn write_mono_samples(
    writer: &mut hound::WavWriter<BufWriter<File>>,
    samples: &[f32],
) -> Result<(), hound::Error> {
    for s in samples {
        writer.write_sample(*s)?;
    }
    Ok(())
}

fn write_mono_as_stereo(
    writer: &mut hound::WavWriter<BufWriter<File>>,
    samples: &[f32],
) -> Result<(), hound::Error> {
    for s in samples {
        writer.write_sample(*s)?;
        writer.write_sample(*s)?;
    }
    Ok(())
}

fn write_interleaved_stereo(
    writer: &mut hound::WavWriter<BufWriter<File>>,
    mic: &[f32],
    spk: &[f32],
) -> Result<(), hound::Error> {
    let max_len = mic.len().max(spk.len());
    for i in 0..max_len {
        let m = mic.get(i).copied().unwrap_or(0.0);
        let s = spk.get(i).copied().unwrap_or(0.0);
        writer.write_sample(m)?;
        writer.write_sample(s)?;
    }
    Ok(())
}

fn finalize_writer(
    writer: &mut Option<hound::WavWriter<BufWriter<File>>>,
    path: Option<&std::path::Path>,
) -> Result<(), hound::Error> {
    if let Some(mut writer) = writer.take() {
        writer.flush()?;
        writer.finalize()?;

        if let Some(p) = path {
            sync_file(p);
        }
    }
    Ok(())
}

fn sync_file(path: &std::path::Path) {
    if let Ok(file) = File::open(path) {
        let _ = file.sync_all();
    }
}

fn sync_dir(path: &std::path::Path) {
    if let Some(parent) = path.parent()
        && let Ok(dir) = File::open(parent)
    {
        let _ = dir.sync_all();
    }
}

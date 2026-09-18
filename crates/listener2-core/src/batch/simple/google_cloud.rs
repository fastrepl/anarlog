use std::path::PathBuf;
use std::time::Duration;

use anlg_audio_utils::Source;
use anlg_transcribe_core::TARGET_SAMPLE_RATE;
use owhisper_client::GoogleCloudAdapter;
use owhisper_interface::{ListenParams, batch::Response};

use super::super::{BatchParams, BatchRunMode, BatchRunOutput};
use super::direct::{
    DIRECT_BATCH_TIMEOUT_FLOOR, merge_segment_responses, run_direct_batch_with_timeout,
};

// Stay below the synchronous API's 60-second limit. Mono 16 kHz PCM segments
// also fit comfortably within the 10 MB request limit after base64 encoding.
const SEGMENT_DURATION: Duration = Duration::from_secs(55);

pub(super) async fn run(
    params: BatchParams,
    mut listen_params: ListenParams,
) -> crate::Result<BatchRunOutput> {
    let path = params.file_path.clone();
    let uploads = tokio::task::spawn_blocking(move || prepare_uploads(&path))
        .await
        .map_err(|_| preparation_failed())?
        .map_err(|_| preparation_failed())?;
    if uploads.channels.is_empty() || uploads.channels[0].is_empty() {
        return Err(crate::BatchFailure::DirectRequestFailed {
            provider: "google_cloud".to_string(),
            message: "This recording has no audio to transcribe.".to_string(),
        }
        .into());
    }

    listen_params.channels = 1;
    let mut channels = Vec::with_capacity(uploads.channels.len());
    for (channel_index, segments) in uploads.channels.iter().enumerate() {
        let mut responses = Vec::with_capacity(segments.len());
        for segment in segments {
            let mut segment_params = params.clone();
            segment_params.file_path = segment.to_string_lossy().into_owned();
            let output = run_direct_batch_with_timeout::<GoogleCloudAdapter>(
                "google_cloud",
                segment_params,
                listen_params.clone(),
                DIRECT_BATCH_TIMEOUT_FLOOR,
            )
            .await?;
            responses.push(output.response);
        }
        let merged = merge_segment_responses(responses, SEGMENT_DURATION);
        for mut channel in merged.results.channels {
            for alternative in &mut channel.alternatives {
                for word in &mut alternative.words {
                    word.channel = channel_index as i32;
                }
            }
            channels.push(channel);
        }
    }

    Ok(BatchRunOutput {
        session_id: params.session_id,
        mode: BatchRunMode::Direct,
        response: Response {
            metadata: serde_json::json!({ "provider": "google_cloud" }),
            results: owhisper_interface::batch::Results { channels },
        },
    })
}

fn preparation_failed() -> crate::BatchFailure {
    crate::BatchFailure::DirectRequestFailed {
        provider: "google_cloud".to_string(),
        message: "Anarlog couldn't prepare this recording for transcription.".to_string(),
    }
}

struct PreparedUploads {
    _directory: tempfile::TempDir,
    channels: Vec<Vec<PathBuf>>,
}

fn prepare_uploads(path: &str) -> Result<PreparedUploads, anlg_audio_utils::Error> {
    let directory = super::super::upload::temporary_audio_directory(path)?;
    let source = anlg_audio_utils::source_from_path(path)?;
    let channel_count = usize::from(u16::from(source.channels()));
    if channel_count > 8 {
        return Err(anlg_audio_utils::Error::TooManyChannels {
            count: channel_count,
        });
    }
    let mut files = (0..channel_count).map(|_| Vec::new()).collect::<Vec<_>>();
    let mut writers = Vec::new();
    let mut segment_frames = 0;
    let max_frames = TARGET_SAMPLE_RATE as usize * SEGMENT_DURATION.as_secs() as usize;
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    // Decode/resample in bounded blocks; never collect the entire meeting in RAM.
    // PCM WAV works on Google's stable v1 endpoint, unlike the shared MP3 splitter.
    anlg_audio_utils::for_each_resampled_channel_block::<_, anlg_audio_utils::Error>(
        source,
        TARGET_SAMPLE_RATE,
        |channels| {
            let mut start = 0;
            while start < channels[0].len() {
                if writers.is_empty() {
                    for (index, channel_files) in files.iter_mut().enumerate() {
                        let path = directory.path().join(format!(
                            "channel-{index}-segment-{}.wav",
                            channel_files.len()
                        ));
                        let writer = hound::WavWriter::create(&path, spec)?;
                        channel_files.push(path);
                        writers.push(writer);
                    }
                }
                let count = (max_frames - segment_frames).min(channels[0].len() - start);
                for (writer, samples) in writers.iter_mut().zip(channels) {
                    for sample in &samples[start..start + count] {
                        writer.write_sample((sample * 32768.0).clamp(-32768.0, 32767.0) as i16)?;
                    }
                }
                start += count;
                segment_frames += count;
                if segment_frames == max_frames {
                    for writer in writers.drain(..) {
                        writer.finalize()?;
                    }
                    segment_frames = 0;
                }
            }
            Ok(())
        },
    )?;
    for writer in writers {
        writer.finalize()?;
    }
    Ok(PreparedUploads {
        _directory: directory,
        channels: files,
    })
}

#[cfg(test)]
mod tests {
    use std::io::{BufWriter, Cursor};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use axum::{
        Json, Router,
        extract::{DefaultBodyLimit, State},
        http::StatusCode,
        routing::post,
    };
    use serde_json::{Value, json};

    use super::super::direct::run_direct_batch_for_adapter_kind;
    use super::*;
    use crate::batch::{BatchProvider, test_fixtures};

    fn recording(seconds: usize, sample_rate: u32) -> tempfile::NamedTempFile {
        let file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
        let mut writer = hound::WavWriter::new(
            BufWriter::new(file.reopen().unwrap()),
            hound::WavSpec {
                channels: 2,
                sample_rate,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();
        for _ in 0..seconds * sample_rate as usize {
            writer.write_sample(0.25_f32).unwrap();
            writer.write_sample(-0.5_f32).unwrap();
        }
        writer.finalize().unwrap();
        file
    }

    #[test]
    fn splits_long_audio_into_pcm_wav_without_losing_channels_or_samples() {
        let source = recording(125, TARGET_SAMPLE_RATE);
        let uploads = prepare_uploads(source.path().to_str().unwrap()).unwrap();
        assert_eq!(uploads.channels.len(), 2);
        let mut paths = Vec::new();
        for (channel, segments) in uploads.channels.iter().enumerate() {
            assert_eq!(segments.len(), 3);
            for (segment, seconds) in segments.iter().zip([55, 55, 15]) {
                let bytes = std::fs::read(segment.as_path()).unwrap();
                assert!(bytes.len().div_ceil(3) * 4 + 4096 < 10_000_000);
                let mut reader = hound::WavReader::new(Cursor::new(bytes)).unwrap();
                assert_eq!(reader.spec().channels, 1);
                assert_eq!(reader.spec().sample_rate, TARGET_SAMPLE_RATE);
                assert_eq!(reader.spec().bits_per_sample, 16);
                assert_eq!(reader.duration(), TARGET_SAMPLE_RATE * seconds);
                let expected = if channel == 0 { 8192 } else { -16384 };
                assert!(
                    reader
                        .samples::<i16>()
                        .all(|sample| sample.unwrap() == expected)
                );
                paths.push(segment.as_path().to_path_buf());
            }
        }
        drop(uploads);
        assert!(paths.iter().all(|path| !path.exists()));
        assert!(source.path().exists());
    }

    #[test]
    fn resamples_short_recordings_and_avoids_empty_boundary_segments() {
        for seconds in [0, 1, 55, 110] {
            let source = recording(seconds, 44_100);
            let uploads = prepare_uploads(source.path().to_str().unwrap()).unwrap();
            for segments in &uploads.channels {
                assert_eq!(segments.len(), seconds.div_ceil(55));
                let frames: u32 = segments
                    .iter()
                    .map(|segment| {
                        let reader = hound::WavReader::open(segment.as_path()).unwrap();
                        assert_eq!(reader.spec().sample_rate, TARGET_SAMPLE_RATE);
                        reader.duration()
                    })
                    .sum();
                assert_eq!(frames, seconds as u32 * TARGET_SAMPLE_RATE);
            }
        }
    }

    async fn server(fail_second: bool) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
        let requests = Arc::new(AtomicUsize::new(0));
        let app = Router::new().route("/v1/speech:recognize", post(
            move |State(requests): State<Arc<AtomicUsize>>, Json(body): Json<Value>| async move {
                let index = requests.fetch_add(1, Ordering::SeqCst);
                assert_eq!(body["config"]["audioChannelCount"], 1);
                assert_eq!(body["config"]["enableSeparateRecognitionPerChannel"], false);
                assert_eq!(body["config"]["model"], "latest_long");
                assert!(body["audio"]["content"].as_str().unwrap().starts_with("UklGR"));
                if fail_second && index == 1 {
                    return (StatusCode::BAD_REQUEST, Json(json!({"error": "rejected"})));
                }
                if index == 1 {
                    return (StatusCode::OK, Json(json!({"results": []})));
                }
                (StatusCode::OK, Json(json!({"results": [{"alternatives": [{
                    "transcript": format!("segment {index}"),
                    "words": [{"word": format!("word{index}"), "startTime": "0.25s", "endTime": "0.75s", "speakerTag": 1}]
                }]}]})))
            }
        )).layer(DefaultBodyLimit::max(3_000_000)).with_state(requests.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{address}/v1"), requests, task)
    }

    #[tokio::test]
    async fn long_recording_dispatch_merges_timestamps_and_preserves_silent_segments() {
        let (url, requests, server) = server(false).await;
        let source = recording(125, TARGET_SAMPLE_RATE);
        let params = test_fixtures::params(
            BatchProvider::GoogleCloud,
            &url,
            source.path().to_str().unwrap(),
        );
        let output = run_direct_batch_for_adapter_kind(
            owhisper_client::AdapterKind::GoogleCloud,
            params,
            ListenParams::default(),
        )
        .await
        .unwrap();
        server.abort();

        assert_eq!(requests.load(Ordering::SeqCst), 6);
        let channels = &output.response.results.channels;
        assert_eq!(channels.len(), 2);
        assert_eq!(
            channels[0].alternatives[0].transcript,
            "segment 0 segment 2"
        );
        for (channel, expected_starts) in [(0, vec![0.25, 110.25]), (1, vec![0.25, 55.25, 110.25])]
        {
            let words = &channels[channel].alternatives[0].words;
            assert_eq!(
                words.iter().map(|word| word.start).collect::<Vec<_>>(),
                expected_starts
            );
            assert!(words.iter().all(|word| word.channel == channel as i32));
            assert!(
                words
                    .windows(2)
                    .all(|pair| pair[0].speaker != pair[1].speaker)
            );
        }
    }

    #[tokio::test]
    async fn failed_segment_stops_the_meeting_without_returning_a_partial_transcript() {
        let (url, requests, server) = server(true).await;
        let source = recording(125, TARGET_SAMPLE_RATE);
        let params = test_fixtures::params(
            BatchProvider::GoogleCloud,
            &url,
            source.path().to_str().unwrap(),
        );
        let result = run_direct_batch_for_adapter_kind(
            owhisper_client::AdapterKind::GoogleCloud,
            params,
            ListenParams::default(),
        )
        .await;
        server.abort();
        assert!(result.is_err());
        assert_eq!(requests.load(Ordering::SeqCst), 2);
        assert!(source.path().exists());
    }
}

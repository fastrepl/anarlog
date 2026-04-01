/// cargo run -p hypr_vad-chunking --example chunking
use std::time::Duration;

use futures_util::StreamExt;
use vad_chunking::VadExt;

#[tokio::main]
async fn main() {
    let out_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/out");
    std::fs::create_dir_all(&out_dir).unwrap();

    let decoder = rodio::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(hypr_data::english_1::AUDIO_MP3_PATH).unwrap(),
    ))
    .unwrap();

    let mut chunks = std::pin::pin!(decoder.speech_chunks(Duration::from_millis(600)));

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut i = 0usize;
    while let Some(Ok(chunk)) = chunks.next().await {
        let path = out_dir.join(format!("{i:03}.wav"));
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for sample in &chunk.samples {
            writer.write_sample(*sample).unwrap();
        }
        writer.finalize().unwrap();

        let duration_ms = chunk.end_timestamp_ms - chunk.start_timestamp_ms;
        println!(
            "{i:03}.wav  {duration_ms:>6}ms  ({} samples)",
            chunk.samples.len()
        );
        i += 1;
    }

    println!("\n{i} chunks written to {}", out_dir.display());
}

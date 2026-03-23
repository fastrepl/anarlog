use vad::{
    earshot::{VoiceActivityDetector as EarshotVad, choose_optimal_frame_size},
    silero_cactus::{Model, VadOptions},
    silero_onnx::{CHUNK_SIZE_16KHZ, SileroVad, pcm_i16_to_f32},
};

fn pcm_bytes_to_i16(bytes: &[u8]) -> Vec<i16> {
    bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

fn earshot_mask(audio: &[u8]) -> Vec<bool> {
    let samples = pcm_bytes_to_i16(audio);
    let frame_size = choose_optimal_frame_size(samples.len());
    let mut detector = EarshotVad::new();
    let mut mask = vec![false; samples.len()];

    for (i, frame) in samples.chunks(frame_size).enumerate() {
        if frame.len() == frame_size {
            let speech = detector.predict_16khz(frame).unwrap();
            let start = i * frame_size;
            for j in start..start + frame_size {
                mask[j] = speech;
            }
        }
    }
    mask
}

fn silero_onnx_mask(audio: &[u8]) -> (Vec<bool>, f32) {
    let samples_i16 = pcm_bytes_to_i16(audio);
    let samples_f32 = pcm_i16_to_f32(&samples_i16);
    let mut model = SileroVad::default();
    let mut mask = vec![false; samples_i16.len()];
    let mut max_prob: f32 = 0.0;

    for (i, chunk) in samples_f32.chunks(CHUNK_SIZE_16KHZ).enumerate() {
        if chunk.len() == CHUNK_SIZE_16KHZ {
            let view = hypr_onnx::ndarray::ArrayView1::from(chunk);
            let prob = model.process_chunk(&view, 16000).unwrap();
            max_prob = max_prob.max(prob);
            let speech = prob > 0.5;
            let start = i * CHUNK_SIZE_16KHZ;
            for j in start..start + CHUNK_SIZE_16KHZ {
                mask[j] = speech;
            }
        }
    }
    (mask, max_prob)
}

fn silero_cactus_mask(audio: &[u8], total_samples: usize) -> Vec<bool> {
    let home = std::env::var("HOME").unwrap();
    let path = format!(
        "{}/Library/Application Support/com.hyprnote.dev/models/cactus/whisper-medium-int8-apple/vad",
        home
    );
    let model = Model::new(&path).unwrap();
    let result = model.vad_pcm(audio, &VadOptions::default()).unwrap();

    let mut mask = vec![false; total_samples];
    for seg in &result.segments {
        let start = seg.start.min(total_samples);
        let end = seg.end.min(total_samples);
        for j in start..end {
            mask[j] = true;
        }
    }
    mask
}

fn agreement(a: &[bool], b: &[bool]) -> f64 {
    let len = a.len().min(b.len());
    let matching = a[..len]
        .iter()
        .zip(&b[..len])
        .filter(|(x, y)| x == y)
        .count();
    matching as f64 / len as f64 * 100.0
}

fn speech_ratio(mask: &[bool]) -> f64 {
    let speech = mask.iter().filter(|&&v| v).count();
    speech as f64 / mask.len() as f64 * 100.0
}

fn compare(name: &str, audio: &[u8]) {
    let total_samples = audio.len() / 2;
    let earshot = earshot_mask(audio);
    let (silero_onnx, silero_onnx_max) = silero_onnx_mask(audio);
    let silero_cactus = silero_cactus_mask(audio, total_samples);

    println!("=== {name} ({:.1}s) ===", total_samples as f64 / 16000.0);
    println!(
        "  Speech %:  earshot={:.1}%  silero_onnx={:.1}% (max_prob={:.4})  silero_cactus={:.1}%",
        speech_ratio(&earshot),
        speech_ratio(&silero_onnx),
        silero_onnx_max,
        speech_ratio(&silero_cactus),
    );
    println!(
        "  Agreement: earshot<>silero_onnx={:.1}%  earshot<>silero_cactus={:.1}%  silero_onnx<>silero_cactus={:.1}%",
        agreement(&earshot, &silero_onnx),
        agreement(&earshot, &silero_cactus),
        agreement(&silero_onnx, &silero_cactus),
    );
    println!();
}

#[test]
fn compare_all_engines() {
    compare("english_1", hypr_data::english_1::AUDIO);
    compare("english_2", hypr_data::english_2::AUDIO);
    compare("english_3", hypr_data::english_3::AUDIO);
    compare("korean_1", hypr_data::korean_1::AUDIO);
    compare("korean_2", hypr_data::korean_2::AUDIO);
}

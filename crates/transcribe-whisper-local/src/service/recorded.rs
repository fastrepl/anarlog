use owhisper_interface::Word2;

pub fn process_recorded(
    model_path: impl AsRef<std::path::Path>,
    audio_path: impl AsRef<std::path::Path>,
) -> Result<Vec<Word2>, crate::Error> {
    let samples = {
        use rodio::Source;

        let source = hypr_audio_utils::source_from_path(audio_path.as_ref()).unwrap();
        let original_sample_rate: u32 = source.sample_rate().into();

        let resampled_samples = if original_sample_rate != 16000 {
            hypr_audio_utils::resample_audio(source, 16000).unwrap()
        } else {
            source.collect()
        };

        hypr_audio_utils::f32_to_i16_samples(&resampled_samples)
    };

    let mut model = hypr_whisper_local::Whisper::builder()
        .model_path(model_path.as_ref().to_str().unwrap())
        .languages(vec![])
        .build()
        .unwrap();

    let mut segmenter = hypr_pyannote_local::segmentation::Segmenter::new(16000).unwrap();
    let segments = segmenter.process(&samples, 16000).unwrap();

    let mut words = Vec::new();

    for segment in segments {
        let audio_f32 = hypr_audio_utils::i16_to_f32_samples(&segment.samples);

        let whisper_segments = model.transcribe(&audio_f32).unwrap();

        for whisper_segment in whisper_segments {
            let start_sec: f64 = segment.start + whisper_segment.start();
            let end_sec: f64 = segment.start + whisper_segment.end();
            let start_ms = (start_sec * 1000.0) as u64;
            let end_ms = (end_sec * 1000.0) as u64;

            let word = Word2 {
                text: whisper_segment.text().to_string(),
                speaker: None,
                confidence: Some(whisper_segment.confidence()),
                start_ms: Some(start_ms),
                end_ms: Some(end_ms),
            };

            // TODO
            words.push(word.clone());
        }
    }

    Ok(words)
}

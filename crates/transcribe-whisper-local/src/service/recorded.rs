use owhisper_interface::{ListenParams, Word2};

pub fn process_recorded(
    model_path: impl AsRef<std::path::Path>,
    audio_path: impl AsRef<std::path::Path>,
) -> Result<Vec<Word2>, crate::Error> {
    process_recorded_with_params(model_path, audio_path, &ListenParams::default())
}

pub fn process_recorded_with_params(
    model_path: impl AsRef<std::path::Path>,
    audio_path: impl AsRef<std::path::Path>,
    params: &ListenParams,
) -> Result<Vec<Word2>, crate::Error> {
    let model = super::load_model(model_path.as_ref())?;
    super::batch::transcribe_recorded_file_with_params(
        &model,
        model_path.as_ref(),
        audio_path.as_ref(),
        params,
    )
}

use owhisper_interface::Word2;

pub fn process_recorded(
    model_path: impl AsRef<std::path::Path>,
    audio_path: impl AsRef<std::path::Path>,
) -> Result<Vec<Word2>, crate::Error> {
    let model = super::load_model(model_path.as_ref())?;
    super::batch::transcribe_recorded_file(&model, model_path.as_ref(), audio_path.as_ref())
}

use dasp::sample::ToSample;

use hypr_onnx::{
    ndarray::{self, Array2},
    ort::{self, session::Session, value::TensorRef},
};

const EMBEDDING_ONNX: &[u8] = include_bytes!("./data/embedding.onnx");

pub struct EmbeddingExtractor {
    session: Session,
}

impl Default for EmbeddingExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl EmbeddingExtractor {
    pub fn new() -> Self {
        let session = hypr_onnx::load_model_from_bytes(EMBEDDING_ONNX).unwrap();
        Self { session }
    }

    pub fn compute(
        &mut self,
        samples: impl Iterator<Item: ToSample<f32>>,
    ) -> Result<Vec<f32>, crate::Error> {
        let samples_f32 = samples.map(|s| s.to_sample_()).collect::<Vec<_>>();

        let features_knf = knf_rs::compute_fbank(&samples_f32)
            .map_err(|s| crate::Error::KnfError(s.to_string()))?;
        let shape = features_knf.shape().to_vec();
        let features: Array2<f32> = ndarray::Array2::from_shape_vec(
            (shape[0], shape[1]),
            features_knf.iter().copied().collect(),
        )
        .map_err(|e| crate::Error::KnfError(e.to_string()))?;

        let features = features.insert_axis(ndarray::Axis(0));
        let inputs = ort::inputs! ["feats" => TensorRef::from_array_view(features.view())?];

        let ort_outs = self.session.run(inputs)?;
        let ort_out = ort_outs.get("embs").unwrap().try_extract_array::<f32>()?;

        let embeddings = ort_out.iter().copied().collect::<Vec<_>>();
        Ok(embeddings)
    }

    pub fn cluster(&self, _n_clusters: usize, embeddings: &[f32]) -> Vec<usize> {
        let assignments = vec![0; embeddings.len()];
        assignments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use dasp::sample::{FromSample, Sample};

    fn get_audio<T: FromSample<f32>>(path: &str) -> Vec<T> {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let p = base.join("src/data").join(path);

        let f32_samples = rodio::Decoder::try_from(std::fs::File::open(p).unwrap())
            .unwrap()
            .collect::<Vec<f32>>();

        f32_samples
            .iter()
            .map(|s| s.to_sample())
            .collect::<Vec<_>>()
    }

    #[test]
    #[ignore]
    fn test_embedding_extractor() {
        use simsimd::SpatialSimilarity;

        let mut extractor = EmbeddingExtractor::new();

        let female_1 = extractor
            .compute(get_audio::<i16>("female_welcome_1.mp3").into_iter())
            .unwrap();
        let male_1 = extractor
            .compute(get_audio::<i16>("male_welcome_1.mp3").into_iter())
            .unwrap();
        let male_2 = extractor
            .compute(get_audio::<i16>("male_welcome_2.mp3").into_iter())
            .unwrap();

        assert_eq!(female_1.len(), male_1.len());
        assert_eq!(female_1.len(), male_2.len());

        assert!(
            (1.0 - f32::cosine(&female_1, &male_1).unwrap())
                < (1.0 - f32::cosine(&male_1, &male_2).unwrap())
        );
        assert!(
            (1.0 - f32::cosine(&female_1, &male_2).unwrap())
                < (1.0 - f32::cosine(&male_2, &male_1).unwrap())
        );
    }

    #[test]
    #[ignore]
    fn test_embedding_extractor_with_f32() {
        let mut extractor = EmbeddingExtractor::new();

        let i16_samples: Vec<i16> = get_audio("female_welcome_1.mp3");
        let embedding_from_i16 = extractor.compute(i16_samples.into_iter()).unwrap();

        let f32_samples: Vec<f32> = get_audio("female_welcome_1.mp3");
        let embedding_from_f32 = extractor.compute(f32_samples.into_iter()).unwrap();

        assert_eq!(embedding_from_i16, embedding_from_f32);
    }
}

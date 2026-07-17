use hypr_onnx::{
    ndarray::{self, ArrayBase, Axis, IxDyn, ViewRepr},
    ort::{self, session::Session, value::TensorRef},
};

const SEGMENTATION_ONNX: &[u8] = include_bytes!("./data/segmentation.onnx");

const FRAME_SIZE: usize = 270;
const FRAME_START: usize = 721;

#[derive(Debug, Clone)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub samples: Vec<i16>,
}

pub struct Segmenter {
    session: Session,
    window_size: usize,
}

impl Segmenter {
    pub fn new(sample_rate: u32) -> Result<Self, crate::Error> {
        let session = hypr_onnx::load_model_from_bytes(SEGMENTATION_ONNX)?;

        Ok(Self {
            session,
            window_size: (sample_rate * 10) as usize,
        })
    }

    pub fn process(
        &mut self,
        samples: &[i16],
        sample_rate: u32,
    ) -> Result<Vec<Segment>, crate::Error> {
        let mut segments = Vec::new();
        let padded = self.pad_samples(samples);
        let mut offset = FRAME_START;
        let mut is_speaking = false;
        let mut start_offset = 0.0;

        for window in padded.chunks(self.window_size) {
            let array = ndarray::Array1::from_iter(window.iter().map(|&x| x as f32))
                .insert_axis(Axis(0))
                .insert_axis(Axis(1))
                .into_dyn();

            let inputs = ort::inputs![TensorRef::from_array_view(array.view())?];
            let run_output = self.session.run(inputs)?;
            let output_tensor = run_output.values().next().unwrap();
            let outputs = output_tensor.try_extract_array::<f32>()?;

            Self::process_outputs(
                outputs,
                &mut is_speaking,
                &mut start_offset,
                &mut offset,
                sample_rate,
                &padded,
                &mut segments,
            )?;
        }

        if is_speaking {
            Self::create_segment(start_offset, offset, sample_rate, &padded, &mut segments)?;
        }

        Ok(segments)
    }

    fn pad_samples(&self, samples: &[i16]) -> Vec<i16> {
        let mut padded = samples.to_vec();
        if !samples.len().is_multiple_of(self.window_size) {
            padded.extend(vec![
                0;
                self.window_size - (samples.len() % self.window_size)
            ]);
        }
        padded
    }

    fn process_outputs(
        outputs: ArrayBase<ViewRepr<&f32>, IxDyn>,
        is_speaking: &mut bool,
        start_offset: &mut f64,
        offset: &mut usize,
        sample_rate: u32,
        padded_samples: &[i16],
        segments: &mut Vec<Segment>,
    ) -> Result<(), crate::Error> {
        for row in outputs.outer_iter() {
            for sub_row in row.axis_iter(Axis(0)) {
                let max_index = Self::find_max_index(sub_row)?;

                if max_index != 0 {
                    if !*is_speaking {
                        *start_offset = *offset as f64;
                        *is_speaking = true;
                    }
                } else if *is_speaking {
                    Self::create_segment(
                        *start_offset,
                        *offset,
                        sample_rate,
                        padded_samples,
                        segments,
                    )?;
                    *is_speaking = false;
                }

                *offset += FRAME_SIZE;
            }
        }
        Ok(())
    }

    fn find_max_index(row: ArrayBase<ViewRepr<&f32>, IxDyn>) -> Result<usize, crate::Error> {
        row.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .ok_or(crate::Error::EmptyRowError)
    }

    fn create_segment(
        start_offset: f64,
        end_offset: usize,
        sample_rate: u32,
        samples: &[i16],
        segments: &mut Vec<Segment>,
    ) -> Result<(), crate::Error> {
        let start = start_offset / sample_rate as f64;
        let end = end_offset as f64 / sample_rate as f64;

        let start_idx = (start * sample_rate as f64) as usize;
        let end_idx = (end * sample_rate as f64) as usize;

        segments.push(Segment {
            start,
            end,
            samples: samples[start_idx..end_idx].to_vec(),
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_segmentation {
        ($name:ident, $audio:expr) => {
            #[test]
            fn $name() {
                let audio: Vec<i16> = $audio
                    .to_vec()
                    .chunks_exact(2)
                    .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect();
                let mut segmenter = Segmenter::new(16000).unwrap();
                let segments = segmenter.process(&audio, 16000).unwrap();

                for segment in segments {
                    println!("{:.2} - {:.2}", segment.start, segment.end);
                }
            }
        };
    }

    test_segmentation!(test_segmentation_english_1, hypr_data::english_1::AUDIO);
    test_segmentation!(test_segmentation_english_2, hypr_data::english_2::AUDIO);
}

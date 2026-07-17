use std::collections::VecDeque;

use audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::Resampler;

/// Wraps a rubato Resampler with queues to enable sample-by-sample input and fixed-size chunk output.
/// Manages buffering between the streaming input and the block-based resampler requirements.
pub struct RubatoChunkResampler<R: Resampler<f32>, const CHANNELS: usize> {
    resampler: R,
    output_chunk_size: usize,
    input_block_size: usize,
    input_queue: VecDeque<f32>,
    rubato_input_buffer: Vec<Vec<f32>>,
    rubato_output_buffer: Vec<Vec<f32>>,
    output_queue: VecDeque<f32>,
}

impl<R: Resampler<f32>, const CHANNELS: usize> RubatoChunkResampler<R, CHANNELS> {
    /// Creates a new wrapper with pre-allocated buffers sized for the resampler's requirements.
    /// Allocates capacity upfront to avoid reallocations during audio processing.
    pub fn new(resampler: R, output_chunk_size: usize, input_block_size: usize) -> Self {
        let rubato_input_buffer = (0..CHANNELS)
            .map(|_| Vec::with_capacity(input_block_size.max(1)))
            .collect();
        let rubato_output_buffer = vec![vec![0.0f32; resampler.output_frames_max()]; CHANNELS];
        let output_queue_capacity = resampler.output_frames_max().max(output_chunk_size);
        let input_queue_capacity = input_block_size.max(1) * CHANNELS;

        Self {
            resampler,
            output_chunk_size,
            input_block_size,
            input_queue: VecDeque::with_capacity(input_queue_capacity),
            rubato_input_buffer,
            rubato_output_buffer,
            output_queue: VecDeque::with_capacity(output_queue_capacity),
        }
    }

    /// Checks whether any resampled output is available.
    pub fn output_is_empty(&self) -> bool {
        self.output_queue.is_empty()
    }

    /// Checks whether at least one full output chunk is ready to be consumed.
    pub fn has_full_chunk(&self) -> bool {
        self.output_queue.len() >= self.output_chunk_size
    }

    /// Extracts exactly one output chunk if available, leaving the rest in the queue.
    /// Returns None if insufficient samples are available.
    pub fn take_full_chunk(&mut self) -> Option<Vec<f32>> {
        if self.output_queue.len() >= self.output_chunk_size {
            Some(self.output_queue.drain(..self.output_chunk_size).collect())
        } else {
            None
        }
    }

    /// Drains all available output samples regardless of chunk boundaries.
    /// Used when flushing remaining samples at stream end.
    pub fn take_all_output(&mut self) -> Option<Vec<f32>> {
        if self.output_queue.is_empty() {
            None
        } else {
            Some(self.output_queue.drain(..).collect())
        }
    }

    /// Checks whether any input samples are waiting to be processed.
    pub fn has_input(&self) -> bool {
        !self.input_queue.is_empty()
    }

    /// Queues a single input sample for resampling.
    pub fn push_sample(&mut self, sample: f32) {
        self.input_queue.push_back(sample);
    }

    /// Processes all complete input blocks currently available in the queue.
    /// Stops when insufficient input remains for another block.
    /// Returns whether any output was produced.
    pub fn process_all_ready_blocks(&mut self) -> Result<bool, crate::Error> {
        let mut produced_output = false;
        loop {
            let frames_needed = self.resampler.input_frames_next();
            if self.input_queue.len() < frames_needed {
                break;
            }
            if self.process_one_block()? {
                produced_output = true;
            }
        }
        Ok(produced_output)
    }

    /// Processes exactly one input block if enough samples are available.
    /// Returns whether output was produced.
    pub fn process_one_block(&mut self) -> Result<bool, crate::Error> {
        let frames_needed = self.resampler.input_frames_next();
        if self.input_queue.len() < frames_needed {
            return Ok(false);
        }
        self.rubato_input_buffer[0].clear();
        self.rubato_input_buffer[0].extend(self.input_queue.drain(..frames_needed));
        let produced_output = self.process_staged_input()?;
        self.rubato_input_buffer[0].clear();
        Ok(produced_output)
    }

    /// Processes an incomplete input block, optionally padding with zeros to meet resampler requirements.
    /// Used for handling the final partial block at stream end when zero_pad is true.
    pub fn process_partial_block(&mut self, zero_pad: bool) -> Result<bool, crate::Error> {
        if self.input_queue.is_empty() {
            return Ok(false);
        }

        let frames_needed = self.resampler.input_frames_next();
        let frames_available = self.input_queue.len();

        if !zero_pad && frames_available < frames_needed {
            return Ok(false);
        }

        self.rubato_input_buffer[0].clear();
        self.rubato_input_buffer[0].extend(self.input_queue.drain(..frames_available));
        if frames_available < frames_needed {
            if zero_pad {
                self.rubato_input_buffer[0].resize(frames_needed, 0.0);
            } else {
                return Ok(false);
            }
        }

        let produced_output = self.process_staged_input()?;
        self.rubato_input_buffer[0].clear();
        Ok(produced_output)
    }

    /// Discards all pending input samples without processing them.
    pub fn clear_input(&mut self) {
        self.input_queue.clear();
    }

    /// Hot-swaps the underlying resampler instance while preserving queue state.
    /// Reallocates buffers and adjusts capacities as needed. Clears input queue to prevent
    /// mixing samples from different configurations.
    pub fn rebind_resampler(
        &mut self,
        resampler: R,
        output_chunk_size: usize,
        input_block_size: usize,
    ) {
        self.resampler = resampler;
        self.output_chunk_size = output_chunk_size;
        self.input_block_size = input_block_size;
        self.rubato_input_buffer = (0..CHANNELS)
            .map(|_| Vec::with_capacity(input_block_size.max(1)))
            .collect();
        self.rubato_output_buffer =
            vec![vec![0.0f32; self.resampler.output_frames_max()]; CHANNELS];

        let desired_output_capacity = self
            .resampler
            .output_frames_max()
            .max(self.output_chunk_size);
        if self.output_queue.capacity() < desired_output_capacity {
            self.output_queue
                .reserve(desired_output_capacity - self.output_queue.capacity());
        }

        let desired_input_capacity = self.input_block_size.max(1) * CHANNELS;
        if self.input_queue.capacity() < desired_input_capacity {
            self.input_queue
                .reserve(desired_input_capacity - self.input_queue.capacity());
        }
        self.clear_input();
    }

    /// Runs the resampler on the staged input buffer and queues the output.
    /// Returns whether any output frames were produced.
    fn process_staged_input(&mut self) -> Result<bool, crate::Error> {
        let frames_in = self.rubato_input_buffer[0].len();
        let frames_out = self.rubato_output_buffer[0].len();
        let input = SequentialSliceOfVecs::new(&self.rubato_input_buffer, CHANNELS, frames_in)
            .expect("input buffer size mismatch");
        let mut output =
            SequentialSliceOfVecs::new_mut(&mut self.rubato_output_buffer, CHANNELS, frames_out)
                .expect("output buffer size mismatch");

        let (_, frames_produced) = self
            .resampler
            .process_into_buffer(&input, &mut output, None)?;
        if frames_produced > 0 {
            self.output_queue.extend(
                self.rubato_output_buffer[0]
                    .iter()
                    .take(frames_produced)
                    .copied(),
            );
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

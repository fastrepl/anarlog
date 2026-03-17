pub(crate) struct CircularBuffer {
    buffer: Vec<f32>,
    block_len: usize,
    block_shift: usize,
}

impl CircularBuffer {
    pub fn new(block_len: usize, block_shift: usize) -> Self {
        Self {
            buffer: vec![0.0f32; block_len],
            block_len,
            block_shift,
        }
    }

    pub fn push_chunk(&mut self, chunk: &[f32]) {
        self.buffer.rotate_left(self.block_shift);
        let copy_len = chunk.len().min(self.block_shift);
        self.buffer
            [self.block_len - self.block_shift..self.block_len - self.block_shift + copy_len]
            .copy_from_slice(&chunk[..copy_len]);

        if copy_len < self.block_shift {
            self.buffer[self.block_len - self.block_shift + copy_len..].fill(0.0);
        }
    }

    pub fn shift_and_accumulate(&mut self, data: &[f32]) {
        self.buffer.rotate_left(self.block_shift);
        self.buffer[self.block_len - self.block_shift..].fill(0.0);

        for (i, &val) in data.iter().enumerate() {
            self.buffer[i] += val;
        }
    }

    pub fn data(&self) -> &[f32] {
        &self.buffer
    }

    pub fn clear(&mut self) {
        self.buffer.fill(0.0);
    }
}

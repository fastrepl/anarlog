use std::collections::VecDeque;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MemoryRetentionPolicy {
    KeepLast(Duration),
}

#[derive(Debug)]
struct EncodedChunk {
    duration: Duration,
    bytes: Vec<u8>,
}

#[derive(Debug)]
pub(super) struct RetainedAudio {
    retention: MemoryRetentionPolicy,
    retained_duration: Duration,
    chunks: VecDeque<EncodedChunk>,
}

impl RetainedAudio {
    pub(super) fn new(retention: MemoryRetentionPolicy) -> Self {
        Self {
            retention,
            retained_duration: Duration::ZERO,
            chunks: VecDeque::new(),
        }
    }

    pub(super) fn push(&mut self, bytes: Vec<u8>, duration: Duration) {
        if bytes.is_empty() && duration.is_zero() {
            return;
        }

        self.push_chunk(EncodedChunk { duration, bytes });
        self.enforce_retention();
        self.debug_assert_invariants();
    }

    pub(super) fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    pub(super) fn iter_bytes(&self) -> impl Iterator<Item = &[u8]> {
        self.chunks.iter().map(|chunk| chunk.bytes.as_slice())
    }

    fn push_chunk(&mut self, chunk: EncodedChunk) {
        self.retained_duration += chunk.duration;
        self.chunks.push_back(chunk);
    }

    fn enforce_retention(&mut self) {
        let MemoryRetentionPolicy::KeepLast(cap) = self.retention;

        while self.retained_duration > cap {
            let Some(chunk) = self.pop_oldest_chunk() else {
                self.retained_duration = Duration::ZERO;
                break;
            };
            self.retained_duration = self.retained_duration.saturating_sub(chunk.duration);
        }
    }

    fn pop_oldest_chunk(&mut self) -> Option<EncodedChunk> {
        self.chunks.pop_front()
    }

    fn debug_assert_invariants(&self) {
        debug_assert_eq!(
            self.retained_duration,
            self.chunks
                .iter()
                .fold(Duration::ZERO, |total, chunk| total + chunk.duration)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retained_bytes(retained: &RetainedAudio) -> Vec<Vec<u8>> {
        retained.iter_bytes().map(ToOwned::to_owned).collect()
    }

    #[test]
    fn keep_last_retention_discards_oldest_chunks() {
        let mut retained =
            RetainedAudio::new(MemoryRetentionPolicy::KeepLast(Duration::from_secs(1)));

        retained.push(vec![1], Duration::from_millis(400));
        retained.push(vec![2], Duration::from_millis(400));
        retained.push(vec![3], Duration::from_millis(400));

        assert_eq!(retained.retained_duration, Duration::from_millis(800));
        assert_eq!(retained.chunks.len(), 2);
        assert_eq!(retained.chunks[0].bytes, vec![2]);
        assert_eq!(retained.chunks[1].bytes, vec![3]);
    }

    #[test]
    fn keeps_chunks_when_total_duration_matches_cap() {
        let mut retained =
            RetainedAudio::new(MemoryRetentionPolicy::KeepLast(Duration::from_secs(1)));

        retained.push(vec![1], Duration::from_millis(400));
        retained.push(vec![2], Duration::from_millis(600));

        assert_eq!(retained.retained_duration, Duration::from_secs(1));
        assert_eq!(retained_bytes(&retained), vec![vec![1], vec![2]]);
    }

    #[test]
    fn ignores_empty_zero_duration_chunks() {
        let mut retained =
            RetainedAudio::new(MemoryRetentionPolicy::KeepLast(Duration::from_secs(1)));

        retained.push(Vec::new(), Duration::ZERO);

        assert!(retained.is_empty());
        assert_eq!(retained.retained_duration, Duration::ZERO);
    }

    #[test]
    fn keeps_zero_duration_flush_bytes() {
        let mut retained =
            RetainedAudio::new(MemoryRetentionPolicy::KeepLast(Duration::from_millis(800)));

        retained.push(vec![1], Duration::from_millis(400));
        retained.push(vec![2], Duration::from_millis(400));
        retained.push(vec![9], Duration::ZERO);

        assert_eq!(retained.retained_duration, Duration::from_millis(800));
        assert_eq!(retained_bytes(&retained), vec![vec![1], vec![2], vec![9]]);
    }

    #[test]
    fn drops_oversized_chunks_when_they_exceed_the_cap_alone() {
        let mut retained =
            RetainedAudio::new(MemoryRetentionPolicy::KeepLast(Duration::from_secs(1)));

        retained.push(vec![1], Duration::from_millis(1500));

        assert!(retained.is_empty());
        assert_eq!(retained.retained_duration, Duration::ZERO);
    }
}

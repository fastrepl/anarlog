use std::collections::VecDeque;

pub(crate) type AudioPair = (Vec<f32>, Vec<f32>);

pub(crate) struct Joiner {
    mic: VecDeque<Vec<f32>>,
    speaker: VecDeque<Vec<f32>>,
}

impl Joiner {
    pub(crate) const MAX_LAG: usize = 4;
    const MAX_QUEUE_SIZE: usize = 30;

    pub(crate) fn new() -> Self {
        Self {
            mic: VecDeque::new(),
            speaker: VecDeque::new(),
        }
    }

    pub(crate) fn push_mic(&mut self, data: Vec<f32>) {
        self.mic.push_back(data);
        if self.mic.len() > Self::MAX_QUEUE_SIZE {
            tracing::warn!("mic_queue_overflow");
            self.mic.pop_front();
        }
    }

    pub(crate) fn push_speaker(&mut self, data: Vec<f32>) {
        self.speaker.push_back(data);
        if self.speaker.len() > Self::MAX_QUEUE_SIZE {
            tracing::warn!("speaker_queue_overflow");
            self.speaker.pop_front();
        }
    }

    pub(crate) fn pop_pair(&mut self) -> Option<AudioPair> {
        if self.mic.front().is_some() && self.speaker.front().is_some() {
            return Some((self.mic.pop_front()?, self.speaker.pop_front()?));
        }

        if self.mic.front().is_some() && self.speaker.is_empty() && self.mic.len() > Self::MAX_LAG {
            let mic = self.mic.pop_front()?;
            let silence = vec![0.0; mic.len()];
            return Some((mic, silence));
        }
        if self.speaker.front().is_some()
            && self.mic.is_empty()
            && self.speaker.len() > Self::MAX_LAG
        {
            let speaker = self.speaker.pop_front()?;
            let silence = vec![0.0; speaker.len()];
            return Some((silence, speaker));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joiner_emits_silence_filled_pair_after_max_lag() {
        let mut joiner = Joiner::new();
        for _ in 0..=Joiner::MAX_LAG {
            joiner.push_mic(vec![0.25, -0.25]);
        }

        let (mic, speaker) = joiner.pop_pair().unwrap();
        assert_eq!(mic, vec![0.25, -0.25]);
        assert_eq!(speaker, vec![0.0, 0.0]);
    }
}

use dasp::sample::ToSample;
use ringbuf::traits::Producer;

pub(crate) const DEFAULT_SCRATCH_LEN: usize = 8192;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PushStats {
    pub(crate) pushed: usize,
    pub(crate) dropped: usize,
}

pub(crate) fn push_interleaved_first_channel_to_ringbuf<S, P>(
    data: &[S],
    channels: usize,
    scratch: &mut [f32],
    producer: &mut P,
) -> PushStats
where
    S: Copy + ToSample<f32>,
    P: Producer<Item = f32>,
{
    if scratch.is_empty() {
        return PushStats::default();
    }

    if channels == 0 {
        return PushStats::default();
    }

    let frame_count = data.len() / channels;
    if frame_count == 0 {
        return PushStats::default();
    }

    let mut offset = 0usize;
    let mut pushed_total = 0usize;
    let mut dropped_total = 0usize;

    if channels == 1 {
        while offset < frame_count {
            let count = (frame_count - offset).min(scratch.len());

            let vacant = producer.vacant_len();
            if vacant == 0 {
                dropped_total += frame_count - offset;
                break;
            }

            let convert_count = count.min(vacant);

            for i in 0..convert_count {
                scratch[i] = data[offset + i].to_sample_();
            }

            let pushed = producer.push_slice(&scratch[..convert_count]);
            pushed_total += pushed;
            dropped_total += count - pushed;

            offset += count;
        }

        return PushStats {
            pushed: pushed_total,
            dropped: dropped_total,
        };
    }

    while offset < frame_count {
        let count = (frame_count - offset).min(scratch.len());

        let vacant = producer.vacant_len();
        if vacant == 0 {
            dropped_total += frame_count - offset;
            break;
        }

        let convert_count = count.min(vacant);

        for i in 0..convert_count {
            scratch[i] = data[(offset + i) * channels].to_sample_();
        }

        let pushed = producer.push_slice(&scratch[..convert_count]);
        pushed_total += pushed;
        dropped_total += count - pushed;

        offset += count;
    }

    PushStats {
        pushed: pushed_total,
        dropped: dropped_total,
    }
}

pub(crate) fn convert_and_push_to_ringbuf<T, P>(
    samples: &[T],
    scratch: &mut [f32],
    producer: &mut P,
    mut convert: impl FnMut(T) -> f32,
) -> PushStats
where
    T: Copy,
    P: Producer<Item = f32>,
{
    if scratch.is_empty() {
        return PushStats::default();
    }

    if samples.is_empty() {
        return PushStats::default();
    }

    let mut offset = 0usize;
    let mut pushed_total = 0usize;
    let mut dropped_total = 0usize;

    while offset < samples.len() {
        let count = (samples.len() - offset).min(scratch.len());

        let vacant = producer.vacant_len();
        if vacant == 0 {
            dropped_total += samples.len() - offset;
            break;
        }

        let convert_count = count.min(vacant);

        for i in 0..convert_count {
            scratch[i] = convert(samples[offset + i]);
        }

        let pushed = producer.push_slice(&scratch[..convert_count]);
        pushed_total += pushed;
        dropped_total += count - pushed;

        offset += count;
    }

    PushStats {
        pushed: pushed_total,
        dropped: dropped_total,
    }
}

pub(crate) fn push_f32_to_ringbuf<P>(data: &[f32], producer: &mut P) -> PushStats
where
    P: Producer<Item = f32>,
{
    if data.is_empty() {
        return PushStats::default();
    }

    let pushed = producer.push_slice(data);
    PushStats {
        pushed,
        dropped: data.len() - pushed,
    }
}

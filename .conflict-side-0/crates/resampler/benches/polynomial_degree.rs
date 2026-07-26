use std::hint::black_box;
use std::time::Instant;

use audioadapter_buffers::direct::SequentialSliceOfVecs;
use resampler::{Async, FixedAsync, PolynomialDegree, Resampler};

fn bench_degree(
    label: &str,
    degree: PolynomialDegree,
    ratio: f64,
    chunk_size: usize,
    iterations: usize,
) {
    let mut resampler =
        Async::<f32>::new_poly(ratio, 2.0, degree, chunk_size, 1, FixedAsync::Input).unwrap();

    let samples: Vec<f32> = (0..chunk_size).map(|i| (i as f32 * 0.01).sin()).collect();
    let input_data = vec![samples];
    let input = SequentialSliceOfVecs::new(&input_data, 1, chunk_size).unwrap();

    let output_frames = resampler.output_frames_max();
    let mut output_data = vec![vec![0.0f32; output_frames]; 1];

    // warmup
    for _ in 0..5 {
        let mut output =
            SequentialSliceOfVecs::new_mut(&mut output_data, 1, output_frames).unwrap();
        resampler
            .process_into_buffer(&input, &mut output, None)
            .unwrap();
    }

    let start = Instant::now();
    for _ in 0..iterations {
        let mut output =
            SequentialSliceOfVecs::new_mut(&mut output_data, 1, output_frames).unwrap();
        black_box(
            resampler
                .process_into_buffer(&input, &mut output, None)
                .unwrap(),
        );
    }
    let elapsed = start.elapsed();

    let per_iter = elapsed / iterations as u32;
    let samples_per_sec = (chunk_size as f64 * iterations as f64) / elapsed.as_secs_f64();
    println!(
        "{label:>12}  total={elapsed:>10.2?}  per_iter={per_iter:>8.2?}  samples/s={samples_per_sec:>12.0}",
    );
}

fn main() {
    let chunk_size = 1024;
    let iterations = 5000;

    // Common STT downsampling: 44100 -> 16000
    let ratio_44_16 = 16000.0 / 44100.0;
    println!(
        "=== 44100 -> 16000 (ratio={ratio_44_16:.4}), chunk_size={chunk_size}, iterations={iterations} ==="
    );
    bench_degree(
        "Linear",
        PolynomialDegree::Linear,
        ratio_44_16,
        chunk_size,
        iterations,
    );
    bench_degree(
        "Cubic",
        PolynomialDegree::Cubic,
        ratio_44_16,
        chunk_size,
        iterations,
    );
    bench_degree(
        "Quintic",
        PolynomialDegree::Quintic,
        ratio_44_16,
        chunk_size,
        iterations,
    );
    println!();

    // Common STT downsampling: 48000 -> 16000
    let ratio_48_16 = 16000.0 / 48000.0;
    println!(
        "=== 48000 -> 16000 (ratio={ratio_48_16:.4}), chunk_size={chunk_size}, iterations={iterations} ==="
    );
    bench_degree(
        "Linear",
        PolynomialDegree::Linear,
        ratio_48_16,
        chunk_size,
        iterations,
    );
    bench_degree(
        "Cubic",
        PolynomialDegree::Cubic,
        ratio_48_16,
        chunk_size,
        iterations,
    );
    bench_degree(
        "Quintic",
        PolynomialDegree::Quintic,
        ratio_48_16,
        chunk_size,
        iterations,
    );
}

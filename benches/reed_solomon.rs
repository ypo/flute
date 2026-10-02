//! Reed-Solomon codec benchmarks
//!
//! `cargo bench --features bench --bench reed_solomon`

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use flute::bench::{mul_add_with, Kernel, ReedSolomon};

/// Encoding symbol length
const E: usize = 1024;

/// (m, k, n)
const CODES: [(u8, usize, usize); 4] = [(8, 32, 48), (8, 64, 96), (8, 200, 255), (16, 200, 255)];

fn create_data(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 31 + i / 7) as u8).collect()
}

fn encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("rs_encode");
    for (m, k, n) in CODES {
        let codec = ReedSolomon::new(m, k, n, E);
        let data = create_data(k * E);
        group.throughput(Throughput::Bytes(data.len() as u64));
        group.bench_function(format!("m{}_k{}_n{}", m, k, n), |b| {
            b.iter(|| codec.encode(black_box(&data)))
        });
    }
    group.finish();
}

fn decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("rs_decode");
    for (m, k, n) in CODES {
        let codec = ReedSolomon::new(m, k, n, E);
        let data = create_data(k * E);
        let encoding_symbols = codec.encode(&data);

        // The first n - k source symbols are lost, and recovered from the n - k repair symbols
        let nb_lost = (n - k).min(k);
        let received: Vec<(u32, &[u8])> = (nb_lost..k)
            .chain(k..k + nb_lost)
            .map(|esi| (esi as u32, encoding_symbols[esi].as_slice()))
            .collect();
        assert_eq!(codec.decode(&received), data);

        group.throughput(Throughput::Bytes(data.len() as u64));
        group.bench_function(format!("m{}_k{}_n{}", m, k, n), |b| {
            b.iter(|| codec.decode(black_box(&received)))
        });
    }
    group.finish();
}

/// GF(2^8) multiply-accumulate dst += c * src of each kernel supported by the CPU
fn gf256_mul_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("gf256_mul_add");
    let src = create_data(E);
    let mut dst = create_data(E + 1)[1..].to_vec();
    group.throughput(Throughput::Bytes(E as u64));
    for kernel in Kernel::available() {
        group.bench_function(format!("{:?}", kernel), |b| {
            b.iter(|| mul_add_with(kernel, black_box(&mut dst), black_box(&src), black_box(0x53)))
        });
    }
    group.finish();
}

criterion_group!(benches, gf256_mul_add, encode, decode);
criterion_main!(benches);

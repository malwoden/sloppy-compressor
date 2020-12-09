use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use sloppycomp::compression::Algorithm;
use sloppycomp::lz77;

fn lz77_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("lz77");
    group.sample_size(10);

    group.bench_function("lz77 compress", |b| {
        let mut input_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        input_file.push("benches/test-files/sloppy-compressor-bench-plaintext");
        let mut file = File::open(input_file).unwrap();

        let mut file_bytes = Vec::new();
        file.read_to_end(&mut file_bytes)
            .expect("Error on file read");

        let compressor = lz77::Lz77Compression {};

        b.iter(|| {
            compressor
                .compress_bytes(&file_bytes, "/tmp/sloppy-compressor-compress-output")
                .unwrap();
        })
    });

    group.bench_function("lz77 decompress", |b| {
        let mut input_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        input_file.push("benches/test-files/sloppy-compressor-bench-compressed");
        let mut file = File::open(input_file).unwrap();

        let mut file_bytes = Vec::new();
        file.read_to_end(&mut file_bytes)
            .expect("Error on file read");

        let compressor = lz77::Lz77Compression {};

        b.iter(|| {
            compressor
                .decompress_bytes(&file_bytes, "/tmp/sloppy-compressor-decompress-output")
                .unwrap();
        })
    });

    group.finish();
}

criterion_group!(benches, lz77_benchmarks);
criterion_main!(benches);

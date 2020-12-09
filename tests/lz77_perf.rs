use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

use sloppycomp::compression::Algorithm;
use sloppycomp::lz77;

#[test]
fn test_compression_size() {
    // test exists so we can monitor and commit changes in optimisations to the
    // compression - slow in debug mode, so run with `cargo test --release`.
    let mut input_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    input_file.push("benches/test-files/sloppy-compressor-bench-plaintext");
    let mut file = File::open(input_file).unwrap();

    let compressor = lz77::Lz77Compression {};
    compressor.compress(file, "/tmp/sloppycomp-ratio-test");

    let compressed_size = std::fs::metadata("/tmp/sloppycomp-ratio-test").unwrap().len();

    assert_eq!(18336826, compressed_size);
}
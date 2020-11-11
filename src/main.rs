use compression::Algorithm;
use std::{env, fs::File, io};

mod block_compress;
mod compression;
mod lz77;

enum CompressionAlgorithm {
    Block,
    Lz77,
}

impl CompressionAlgorithm {
    fn create(&self) -> Box<dyn Algorithm> {
        match *self {
            CompressionAlgorithm::Block => Box::new(block_compress::BlockCompression {}),
            CompressionAlgorithm::Lz77 => Box::new(lz77::Lz77Compression {}),
        }
    }
}

/// a really rubbish file compressor.
///
/// Compress: `./sloppy-compressor lz77 compress ~/file/input.name ~/file/output.name`
///
/// To decompress - `./sloppy-compressor lz77 decompress ~/file/input.name ~/file/output.name`
///
/// The program ignores most error checking and will overwrite files without warning.
fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let algo = &args[1];
    let compress_mode = &args[2] == "compress";
    let path = &args[3];
    let output_path = &args[4];
    println!("{:?}", args);

    let algorithm = match algo.as_str() {
        "block" => CompressionAlgorithm::Block,
        "lz77" => CompressionAlgorithm::Lz77,
        _ => panic!("Unknown compression algorithm"),
    };
    let compressor = algorithm.create();

    let file = File::open(path)?;

    if compress_mode {
        compressor
            .compress(file, path)
            .expect("Error on compression");
    } else {
        compressor
            .decompress(file, output_path)
            .expect("Error on decompression");
    }

    Ok(())
}

use std::{env, fs::File, io};

use sloppycomp::block_compress;
use sloppycomp::compression;
use sloppycomp::lz77;

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

    // I wonder what the compiler outputs for this?
    // We could avoid the heap alloc if we just had if/else blocks.
    let compressor: Box<dyn compression::Algorithm> = match algo.as_str() {
        "block" => Box::new(block_compress::BlockCompression {}),
        "lz77" => Box::new(lz77::Lz77Compression {}),
        _ => panic!("Unknown compression algorithm"),
    };

    let file = File::open(path)?;

    if compress_mode {
        compressor
            .compress(file, output_path)
            .expect("Error on compression");
    } else {
        compressor
            .decompress(file, output_path)
            .expect("Error on decompression");
    }

    Ok(())
}

use compression::Algorithm;
use std::{env, fs::File, io};

mod block_compress;
mod compression;
mod lz77;

/// a really rubbish file compressor.
///
/// Compress: `./sloppy-compressor compress ~/file/input.name ~/file/output.name`
///
/// To decompress - `./sloppy-compressor decompress ~/file/input.name ~/file/output.name`
///
/// The program ignores most error checking and will overwrite files without warning.
fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let compress_mode = &args[1] == "compress";
    let path = &args[2];
    let output_path = &args[3];
    println!("{:?}", args);

    let compressor = block_compress::BlockCompression {};
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

use std::env;
mod block_compress;
mod lz77;

/// a really rubbish file compressor.
///
/// Compress: `./sloppy-compressor compress ~/file/path`
/// will result in a file `~/file/path.scomp`
///
/// To decompress - `./sloppy-compressor decompress ~/file/path.scomp`
///
/// The program ignores most error checking and will overwrite files without warning.
fn main() {
    let args: Vec<String> = env::args().collect();
    let compress_mode = &args[1] == "compress";
    let path = &args[2];
    println!("{:?}", args);

    if compress_mode {
        block_compress::compress(path).expect("Error on compression");
    } else {
        block_compress::decompress(path).expect("Error on decompression");
    }
}

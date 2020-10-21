use hex;
use md5;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::{collections::HashMap, io::BufReader};

const BLOCK_SIZE: usize = 128;

#[derive(Serialize, Deserialize, Debug)]
struct Compressed {
    block_map: Vec<u32>,
    blocks: Vec<Vec<u8>>,
}

/// a really rubbish file compressor. Looks for matching blocks within the file.
///
/// Compress: `./sloppy-compressor compress ~/file/path`
/// will result in a file `~/file/path.scomp`
///
/// To decompress - `./sloppy-compressor decompress ~/file/path.scomp`
///
/// The program ignores most error checking and will overwrite files without warning.
fn main() -> () {
    let args: Vec<String> = env::args().collect();
    let compress_mode = &args[1] == "compress";
    let path = &args[2];
    println!("{:?}", args);

    if compress_mode {
        compress(path).expect("Error on compression");
    } else {
        decompress(path).expect("Error on decompression");
    }
}

fn compress(file_path: &str) -> io::Result<()> {
    let mut file = File::open(file_path)?;
    let mut buffer = [0; BLOCK_SIZE];

    let mut block_map = Vec::new();
    let mut block_hashes = HashMap::new();
    let mut blocks: Vec<Vec<u8>> = Vec::new();

    println!("Original Size: {}", file.metadata()?.len());

    loop {
        let n = file.read(&mut buffer[..])?;
        if n == 0 {
            break;
        }

        let b = &buffer[..n];
        let strong = strong_hash(b);

        if block_hashes.contains_key(&strong) {
            let index = block_hashes.get(&strong).unwrap();
            block_map.push(*index);
        } else {
            blocks.push(b.to_vec());
            let new_block_index = (blocks.len() - 1) as u32;
            block_hashes.insert(strong, new_block_index);
            block_map.push(new_block_index);
        }
    }

    let compressed = Compressed {
        block_map,
        blocks: blocks,
    };

    write_compressed(&compressed, file_path)
}

fn write_compressed(compressed: &Compressed, original_file_name: &str) -> io::Result<()> {
    let encoded = bincode::serialize(&compressed).unwrap();
    let mut out_file_path = original_file_name.to_string();
    out_file_path.push_str(".scomp");

    let mut out_file = File::create(out_file_path)?;
    out_file.write_all(&encoded)?;

    println!("Compressed Size: {}", encoded.len());
    Ok(())
}

fn decompress(compressed_file_name: &str) -> io::Result<()> {
    let compressed_file = File::open(compressed_file_name)?;
    let buf_reader = BufReader::new(compressed_file);
    let compressed: Compressed = bincode::deserialize_from(buf_reader).unwrap();
    let decomp_file_name = compressed_file_name.to_string();

    let mut f = File::create(decomp_file_name.strip_suffix(".scomp").unwrap())?;
    for index in compressed.block_map {
        let block_data = &compressed.blocks[index as usize];
        f.write_all(&block_data)?;
    }
    Ok(())
}

fn strong_hash(buf: &[u8]) -> String {
    let hash_digest = md5::compute(buf);
    hex::encode(hash_digest.0)
}

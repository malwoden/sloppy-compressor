use serde::{Deserialize, Serialize};
use std::collections::hash_map::Entry;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::{collections::HashMap, io::BufReader};

use super::compression;

const BLOCK_SIZE: usize = 128;

pub struct BlockCompression {}

/// Compresses a file by looking for matching block patterns.block_compress
///
/// Inspired by the rsync algo - this algorithm reads in a file block by block, taking a
/// checksum of the block. When we take a block's checksum we check it against all other previously
/// seen checksums. If we find a hit then we store a reference to the previous block, rather than storing
/// the raw data again.
///
/// This is a poor compression method - there is a good chance that it makes your file larger
/// due to the overheads of the data structure on disk.
impl compression::Algorithm for BlockCompression {
  fn compress(&self, mut file: File, output_file_path: &str) -> io::Result<()> {
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
      match block_hashes.entry(strong) {
        Entry::Occupied(entry) => block_map.push(*entry.get()),
        Entry::Vacant(entry) => {
          blocks.push(b.to_vec());
          let new_block_index = (blocks.len() - 1) as u32;
          entry.insert(new_block_index);
          block_map.push(new_block_index);
        }
      };
    }
    let compressed = Compressed { block_map, blocks };
    write_compressed(&compressed, output_file_path)
  }

  fn decompress(&self, compressed_file: File, output_file_path: &str) -> io::Result<()> {
    let buf_reader = BufReader::new(compressed_file);
    let compressed: Compressed = bincode::deserialize_from(buf_reader).unwrap();
    let mut file = File::create(output_file_path)?;
    for index in compressed.block_map {
      let block_data = &compressed.blocks[index as usize];
      file.write_all(&block_data)?;
    }
    Ok(())
  }
}

#[derive(Serialize, Deserialize, Debug)]
struct Compressed {
  block_map: Vec<u32>,
  blocks: Vec<Vec<u8>>,
}

fn write_compressed(compressed: &Compressed, output_file_path: &str) -> io::Result<()> {
  let encoded = bincode::serialize(&compressed).unwrap();

  let mut out_file = File::create(output_file_path)?;
  out_file.write_all(&encoded)?;

  println!("Compressed Size: {}", encoded.len());
  Ok(())
}

fn strong_hash(buf: &[u8]) -> String {
  let hash_digest = md5::compute(buf);
  hex::encode(hash_digest.0)
}

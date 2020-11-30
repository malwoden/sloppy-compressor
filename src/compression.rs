use std::fs::File;
use std::io;
use std::io::prelude::*;

pub trait Algorithm {
    fn compress(&self, file: File, output_file_path: &str) -> io::Result<()>;
    fn decompress(&self, compressed_file: File, output_file_path: &str) -> io::Result<()>;
}

pub fn write_compressed<T>(compressed: &T, output_file_path: &str) -> io::Result<()>
where
    T: serde::Serialize,
{
    let encoded = bincode::serialize(compressed).unwrap();
    write_to_new_file(&encoded, output_file_path)
}

pub fn write_to_new_file(read_from: &Vec<u8>, output_file_path: &str) -> io::Result<()> {
    let mut out_file = File::create(output_file_path)?;
    out_file.write_all(read_from)
}

# Sloppy-compressor

This project contains the implementations for 2 compression algorithms.

One algorithm is an implementation of lz77, the other is my own creation that more often than
not makes files larger than when you originally started.

This is a toy project for getting up to speed on writing Rust and isn't intended for any
serious use.


## lz77

The lz77 compression is implemented along with the serialisation format described by
https://en.wikipedia.org/wiki/Lempel%E2%80%93Ziv%E2%80%93Stac.

Some optimisations have been made to speed up the compression but it is not exhaustive.
The majority of time on a compression pass is spent looking back in the search buffer for
the longest matching slice - so this would be a sound place to start focusing on speed, e.g
keeping a 2 byte lookup collection, rather than the single byte lookup we have in
IndexableByteWindow currently - we can only compress byte matches > 1.

As the compression itself improves, it would make sense to move the disk writing to happen
in parallel to compression calculations.

Future plan for this was to implement DEFLATE with Huffman coding etc.


## Block compressor

This is a simple compression scheme that looks for matching whole blocks of content within
a file. Matching blocks are stored as a reference rather than the whole file itself.

This uses serde to serialise and deserialise the built data structure, the overhead of this
will result in the 'compressed' file being larger than the source file unless a good amount
of block matches are found.


# Profiling

Criterion has been used to track improvements. Flamegraphs are used to help identify problem areas.


## Benchmarks

Criterion benchmarks can be run using

`cargo bench`

Once you pull master, save some benchmarks:

`cargo bench --bench lz77_benchmarks -- --save-baseline master`

Once you have made some code changes, check them against that saved baseline:

`cargo bench --bench lz77_benchmarks -- --baseline master`

If you are happy with the changes, save the new master as the baseline, ready for future changes:

`cargo bench --bench lz77_benchmarks -- --save-baseline master`


## Flamegraph

Using: https://github.com/flamegraph-rs/flamegraph

Quick Setup:

```
sudo apt install -y linux-tools-common linux-tools-generic
cargo install flamegraph
cargo build --release && flamegraph  ./target/release/sloppy-compressor lz77 compress ~/some/file/to/compress ./out.wiki
```

Then view the generated svg.

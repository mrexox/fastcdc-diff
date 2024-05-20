#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

use similar::utils::diff_slices;
use similar::{Algorithm, ChangeTag};

use arrayref::array_ref;
use fastcdc::v2020::StreamCDC;

#[macro_use]
extern crate napi_derive;

const VERSION: u8 = 0;

#[derive(Debug, Eq, PartialEq)]
struct Signature {
  version: u8,
  chunks: Vec<Chunk>,
}

#[derive(Debug, Hash, Clone)]
struct Chunk {
  hash: u64,
  offset: u64,
  length: usize,
}

impl PartialEq for Chunk {
  fn eq(&self, other: &Self) -> bool {
    self.hash == other.hash && self.length == other.length
  }
}

impl Eq for Chunk {}

impl PartialOrd for Chunk {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.offset.cmp(&other.offset))
  }
}

impl Ord for Chunk {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.offset.cmp(&other.offset)
  }
}

impl Signature {
  /// Calculates a signature based on FastCDC algorithm on a raw data.
  fn new<R: Read>(source: R) -> Result<Self> {
    let chunker = StreamCDC::new(source, 4096, 16384, 65535);

    let mut chunks: Vec<Chunk> = Vec::new();

    for result in chunker {
      let chunk = result.unwrap();
      chunks.push(Chunk {
        hash: chunk.hash,
        offset: chunk.offset,
        length: chunk.length,
      });
    }

    Ok(Self {
      version: VERSION,
      chunks,
    })
  }

  /// Creates a signature from binary data saved by `write` function.
  fn from(vec: &[u8]) -> Result<Self> {
    let version = vec[0];
    let numchunks = usize::from_be_bytes(*array_ref![vec, 1, 8]);
    let mut offset = 9;
    let mut chunks = Vec::with_capacity(numchunks);
    for _i in 0..numchunks {
      chunks.push(Chunk {
        hash: u64::from_be_bytes(*array_ref![vec, offset, 8]),
        offset: u64::from_be_bytes(*array_ref![vec, offset + 8, 8]),
        length: usize::from_be_bytes(*array_ref![vec, offset + 16, 8]),
      });

      offset += 24;
    }

    Ok(Self { version, chunks })
  }

  fn write<W: Write>(&self, dest: &mut W) -> Result<()> {
    dest.write_all(&[self.version])?;
    dest.write_all(self.chunks.len().to_be_bytes().as_ref())?;

    for chunk in self.chunks.iter() {
      dest.write_all(chunk.hash.to_be_bytes().as_ref())?;
      dest.write_all(chunk.offset.to_be_bytes().as_ref())?;
      dest.write_all(chunk.length.to_be_bytes().as_ref())?;
    }

    dest.flush()?;

    Ok(())
  }
}

/// Calculate the signature and write it to `dest`.
#[napi]
pub fn signature_to_file(source: String, dest: String) -> Result<()> {
  if !Path::new(&source).exists() {
    return Err(Error::from_reason(format!("file {source} does not exist")));
  }

  let source = File::open(source).unwrap();
  let mut dest = File::create(dest).unwrap();

  let signature = Signature::new(source).unwrap();
  signature.write(&mut dest).unwrap();

  Ok(())
}

/// Calculate the signature and return it as a Buffer.
#[napi]
pub fn signature(source: String) -> Result<Buffer> {
  if !Path::new(&source).exists() {
    return Err(Error::from_reason(format!("file {source} does not exist")));
  }

  let source = File::open(source).unwrap();
  let signature = Signature::new(source).unwrap();
  let mut dest = Vec::new();
  signature.write(&mut dest).unwrap();

  Ok(dest.into())
}

#[napi]
pub fn diff(source: String, dest: String) -> Result<()> {
  let source_file = File::open(source.clone()).unwrap();
  let source_sig = Signature::new(source_file).unwrap();

  let dest_file = File::open(dest).unwrap();
  let dest_sig = Signature::new(dest_file).unwrap();

  // Measure diffing
  println!("source: {}", source_sig.chunks.len());
  println!("dest: {}", dest_sig.chunks.len());
  use std::time::Instant;
  let now = Instant::now();

  let res = diff_slices(Algorithm::Myers, &source_sig.chunks, &dest_sig.chunks);

  let elapsed = now.elapsed();
  println!("Myers, Elapsed: {:.2?}", elapsed);
  dbg!(res.len());
  for action in res.iter() {
    println!("{} ({})", action.0, action.1.len());
  }

  Ok(())
}

#[napi]
pub fn diff_sig(sig_source: String, dest: String) -> Result<()> {
  let sig_data = fs::read(sig_source).unwrap();
  let source_sig = Signature::from(&sig_data).unwrap();

  let dest_file = File::open(dest).unwrap();
  let dest_sig = Signature::new(dest_file).unwrap();

  let diff_res = diff_slices(Algorithm::Myers, &source_sig.chunks, &dest_sig.chunks);
  for action in diff_res.iter() {
    match action.0 {
      ChangeTag::Delete => {}
      ChangeTag::Insert => {
        let mut size: usize = 0;
        action.1.into_iter().for_each(|ch| size += ch.length);
        println!("+ {} bytes", size);
      }
      ChangeTag::Equal => {
        let first = &action.1[0];
        let last = &action.1[action.1.len() - 1];
        println!(
          "= {} -> {}",
          first.offset,
          last.offset + (last.length as u64)
        );
      }
    }
  }
  Ok(())
}

#[test]
fn test_signature_serialization() {
  let data: Vec<u8> = (0..100500).map(|_| rand::random::<u8>()).collect();

  let sig = Signature::new(&data[..]).unwrap();
  let mut serialized_data = Vec::new();
  sig.write(&mut serialized_data).unwrap();

  let sig_re = Signature::from(&serialized_data).unwrap();

  assert_eq!(sig, sig_re);
}

// #[napi]
// pub fn apply(diff: String, target: String) -> Result<()> {
//   Ok(())
// }

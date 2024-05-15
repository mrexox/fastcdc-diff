#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

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

#[derive(Debug, Clone, Eq, PartialEq)]
struct Chunk {
  hash: u64,
  offset: u64,
  length: usize,
}

impl Signature {
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

  fn new(source: impl Read) -> Result<Self> {
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

  fn write(&self, dest: &mut impl Write) -> Result<()> {
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

#[napi]
pub fn signature(source: String, dest: String) -> Result<()> {
  if !Path::new(&source).exists() {
    return Err(Error::from_reason(format!("file {source} does not exist")));
  }

  let source = File::open(source).unwrap();
  let mut dest = File::create(dest).unwrap();

  let signature = Signature::new(source).unwrap();
  signature.write(&mut dest).unwrap();

  Ok(())
}

#[napi]
pub fn signature_print(source: String) -> Result<()> {
  if !Path::new(&source).exists() {
    return Err(Error::from_reason(format!("file {source} does not exist")));
  }

  let source = File::open(source).unwrap();
  let signature = Signature::new(source).unwrap();

  println!("{:?}", signature);
  Ok(())
}

#[napi]
pub fn diff(path: String, signature: String, dest: String) -> Result<()> {
  let sig_data = fs::read(signature).unwrap();
  let source_sig = Signature::from(&sig_data);
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

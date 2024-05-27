use arrayref::array_ref;
use fastcdc::v2020::StreamCDC;
use std::io::{self, Read, Write};

pub const VERSION: u8 = 0;
pub const DEFAULT_MIN_SIZE: u32 = 4096;
pub const DEFAULT_AVG_SIZE: u32 = 16384;
pub const DEFAULT_MAX_SIZE: u32 = 65535;

#[derive(Debug, Eq, PartialEq)]
pub struct Signature {
  pub version: u8,
  pub min_size: u32,
  pub avg_size: u32,
  pub max_size: u32,
  pub chunks: Vec<Chunk>,
}

#[derive(Debug, Clone)]
pub struct Chunk {
  pub hash: blake3::Hash,
  pub offset: u64,
  pub length: usize,
}

impl PartialEq for Chunk {
  // Chunks are equal when they have similar data hashes. Blake3 strong hashing guarantees this.
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

impl Signature {
  /// Calculates a signature using FastCDC to determine the data chunks and Blake3 to calculate
  /// strong hashes.
  pub fn calculate(
    source: &mut impl Read,
    min_size: u32,
    avg_size: u32,
    max_size: u32,
  ) -> Result<Self, io::Error> {
    let chunker = StreamCDC::new(source, min_size, avg_size, max_size);
    let mut chunks: Vec<Chunk> = Vec::new();

    for result in chunker {
      let chunk = result?;
      let hash = blake3::hash(&chunk.data);

      chunks.push(Chunk {
        hash,
        offset: chunk.offset,
        length: chunk.length,
      });
    }

    Ok(Self {
      version: VERSION,
      min_size,
      avg_size,
      max_size,
      chunks,
    })
  }

  /// Loads signature from raw data.
  pub fn load(vec: &[u8]) -> Self {
    let version = vec[0];
    let min_size = u32::from_be_bytes(*array_ref![vec, 1, 4]);
    let avg_size = u32::from_be_bytes(*array_ref![vec, 5, 4]);
    let max_size = u32::from_be_bytes(*array_ref![vec, 9, 4]);
    let numchunks = usize::from_be_bytes(*array_ref![vec, 13, 8]);
    let mut offset = 21;
    let mut chunks = Vec::with_capacity(numchunks);
    for _i in 0..numchunks {
      chunks.push(Chunk {
        hash: (*array_ref![vec, offset, 32]).into(),
        offset: u64::from_be_bytes(*array_ref![vec, offset + 32, 8]),
        length: usize::from_be_bytes(*array_ref![vec, offset + 40, 8]),
      });

      offset += 48;
    }

    Self {
      version,
      min_size,
      avg_size,
      max_size,
      chunks,
    }
  }

  pub fn write<W: Write>(&self, dest: &mut W) -> Result<(), io::Error> {
    dest.write_all(&[self.version])?;
    dest.write_all(self.min_size.to_be_bytes().as_ref())?;
    dest.write_all(self.avg_size.to_be_bytes().as_ref())?;
    dest.write_all(self.max_size.to_be_bytes().as_ref())?;
    dest.write_all(self.chunks.len().to_be_bytes().as_ref())?;

    for chunk in self.chunks.iter() {
      dest.write_all(chunk.hash.as_bytes().as_ref())?;
      dest.write_all(chunk.offset.to_be_bytes().as_ref())?;
      dest.write_all(chunk.length.to_be_bytes().as_ref())?;
    }

    dest.flush()?;

    Ok(())
  }
}

#[test]
fn test_signature_serialization() {
  use std::io::Cursor;
  let data: Vec<u8> = (0..100500).map(|_| rand::random::<u8>()).collect();
  let mut buffer = Cursor::new(&data[..]);
  let sig = Signature::calculate(
    &mut buffer,
    DEFAULT_MIN_SIZE,
    DEFAULT_AVG_SIZE,
    DEFAULT_MAX_SIZE,
  )
  .unwrap();
  let mut serialized_data = Vec::new();
  sig
    .write(&mut serialized_data)
    .expect("can't serialize the signature");

  let sig_re = Signature::load(&serialized_data);
  assert_eq!(sig, sig_re);
}

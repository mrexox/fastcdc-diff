#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use std::fmt;
use std::fs::{self, File};
use std::io::{copy, ErrorKind, Read, Seek, SeekFrom, Write};
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
  hash: [u8; 32],
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
  fn new<R: Read + Seek>(mut source: R) -> Result<Self> {
    use std::time::Instant;
    let chunker = StreamCDC::new(&mut source, 4096, 16384, 65535);
    let mut chunks: Vec<Chunk> = Vec::new();

    let now = Instant::now();
    for result in chunker {
      let chunk = result.unwrap();

      chunks.push(Chunk {
        hash: [0; 32],
        offset: chunk.offset,
        length: chunk.length,
      });
    }
    let elapsed = now.elapsed();
    println!("FastCDC completed: {:.2?}", elapsed);

    let now = Instant::now();

    chunks.iter_mut().for_each(|chunk| {
      source.seek(SeekFrom::Start(chunk.offset)).unwrap();
      let mut buf: Vec<u8> = vec![0; chunk.length];
      source.read_exact(&mut buf).unwrap();
      let hash = blake3::hash(&buf);
      chunk.hash = hash.into();
    });

    let elapsed = now.elapsed();
    println!("Blake2 calculated: {:.2?}", elapsed);

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
        hash: array_ref![vec, offset, 32].clone(),
        offset: u64::from_be_bytes(*array_ref![vec, offset + 32, 8]),
        length: usize::from_be_bytes(*array_ref![vec, offset + 40, 8]),
      });

      offset += 48;
    }

    Ok(Self { version, chunks })
  }

  fn write<W: Write>(&self, dest: &mut W) -> Result<()> {
    dest.write_all(&[self.version])?;
    dest.write_all(self.chunks.len().to_be_bytes().as_ref())?;

    for chunk in self.chunks.iter() {
      dest.write_all(chunk.hash.as_ref())?;
      dest.write_all(chunk.offset.to_be_bytes().as_ref())?;
      dest.write_all(chunk.length.to_be_bytes().as_ref())?;
    }

    dest.flush()?;

    Ok(())
  }
}

impl fmt::Display for Signature {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{} chunks: {}", self.version, self.chunks.len())?;
    for chunk in self.chunks.iter() {
      write!(f, "{} {} {}\n", chunk.hash[0], chunk.offset, chunk.length)?;
    }
    Ok(())
  }
}

/// Calculate the signature and write it to `dest`.
#[napi]
pub fn signature_to_file(source: String, dest: String) -> Result<()> {
  if !Path::new(&source).exists() {
    return Err(Error::from_reason(format!("file {source} does not exist")));
  }

  let mut source = File::open(source).unwrap();
  let mut dest = File::create(dest).unwrap();

  let signature = Signature::new(&mut source).unwrap();
  signature.write(&mut dest).unwrap();

  Ok(())
}

/// Calculate the signature and return it as a Buffer.
#[napi]
pub fn signature(source: String) -> Result<Buffer> {
  if !Path::new(&source).exists() {
    return Err(Error::from_reason(format!("file {source} does not exist")));
  }

  let mut source = File::open(source).unwrap();
  let signature = Signature::new(&mut source).unwrap();
  let mut dest = Vec::new();
  signature.write(&mut dest).unwrap();

  // println!("{}", signature);

  Ok(dest.into())
}

/// Generate simple diff format:
///
/// VERSION(u8) - a diff file version for compatibility checking
/// OPERATION(u8) - 0/1, 0 means equal, 1 means insert
/// DATA:
///   for 0:
///     START OFFSET(u64) - offset of the file A to copy from
///     END OFFSET(u64) - offset of the file A to copy until
///   for 1:
///     SIZE(usize) - the number of bytes
///     BYTES([u8]) - the raw binary data from file B to be insterted
#[napi]
pub fn diff(a: String, b: String, diff_path: String) -> Result<()> {
  let mut a_file = File::open(a).unwrap();
  let a_sig = Signature::new(&mut a_file).unwrap();

  let mut b_file = File::open(b).unwrap();
  let b_sig = Signature::new(&mut b_file).unwrap();

  let res = diff_slices(Algorithm::Myers, &a_sig.chunks, &b_sig.chunks);

  let mut diff_file = File::create(diff_path).unwrap();
  diff_file.write_all(&[VERSION]).unwrap();

  for action in res.iter() {
    match action.0 {
      ChangeTag::Delete => {}
      ChangeTag::Insert => {
        let start_chunk = &action.1[0];
        let end_chunk = &action.1[action.1.len() - 1];
        diff_file.write_all(&[1]).unwrap();
        // let mut buf = vec![
        //   0;
        //   (end_chunk.offset + (end_chunk.length as u64) - start_chunk.offset)
        //     .try_into()
        //     .unwrap()
        // ];

        // b_file.read_exact(&mut buf).unwrap();
        // diff_file
        //   .write_all(buf.len().to_be_bytes().as_ref())
        //   .unwrap();
        // diff_file.write_all(&buf).unwrap();

        b_file.seek(SeekFrom::Start(start_chunk.offset)).unwrap();
        let size = end_chunk.offset + end_chunk.length as u64 - start_chunk.offset;
        diff_file.write_all(size.to_be_bytes().as_ref()).unwrap();
        let mut chunk = b_file.take(size);
        copy(&mut chunk, &mut diff_file).unwrap();
        b_file = chunk.into_inner();

        println!(
          "+ {} - {} ({})",
          start_chunk.offset,
          end_chunk.offset + end_chunk.length as u64,
          size
        );
      }
      ChangeTag::Equal => {
        let start_chunk = &action.1[0];
        let end_chunk = &action.1[action.1.len() - 1];

        diff_file.write_all(&[0]).unwrap();
        diff_file
          .write_all(start_chunk.offset.to_be_bytes().as_ref())
          .unwrap();
        diff_file
          .write_all(
            (end_chunk.offset + end_chunk.length as u64)
              .to_be_bytes()
              .as_ref(),
          )
          .unwrap();

        println!(
          "= {} - {} ({})",
          start_chunk.offset,
          end_chunk.offset + end_chunk.length as u64,
          end_chunk.offset + end_chunk.length as u64 - start_chunk.offset
        );
      }
    }
  }

  Ok(())
}

#[napi]
pub fn apply(diff_path: String, a: String, result: String) -> Result<()> {
  let mut diff_file = File::open(diff_path).unwrap();

  let mut a_file = File::open(a).unwrap();
  let mut res_file = File::create(result).unwrap();

  let mut buf: [u8; 1] = [0; 1];

  diff_file.read_exact(&mut buf).unwrap();
  if buf[0] != VERSION {
    return Err(Error::from_reason(format!(
      "unsupported diff version: {}, wanted: {}",
      buf[0], VERSION
    )));
  }

  let mut u64buf: [u8; 8] = [0; 8];

  loop {
    if let Err(err) = diff_file.read_exact(&mut buf) {
      if err.kind() == ErrorKind::UnexpectedEof {
        break;
      }

      return Err(Error::from_reason(format!("unexpected error: {}", err)));
    }

    match buf[0] {
      0 => {
        diff_file.read_exact(&mut u64buf).unwrap();
        let start = u64::from_be_bytes(u64buf);
        diff_file.read_exact(&mut u64buf).unwrap();
        let end = u64::from_be_bytes(u64buf);

        a_file.seek(SeekFrom::Start(start)).unwrap();

        let mut chunk = a_file.take(end - start);

        // let mut data = vec![0; (end - start).try_into().unwrap()];
        // a_file.read_exact(&mut data).unwrap();
        // res_file.write_all(&data).unwrap();
        copy(&mut chunk, &mut res_file).unwrap();

        a_file = chunk.into_inner();
      }
      1 => {
        diff_file.read_exact(&mut u64buf).unwrap();
        let size = usize::from_be_bytes(u64buf);
        // let mut data = vec![0; size];
        // diff_file.read_exact(&mut data).unwrap();
        // res_file.write_all(&data).unwrap();

        let mut chunk = diff_file.take(size as u64);
        copy(&mut chunk, &mut res_file).unwrap();
        diff_file = chunk.into_inner();
      }
      _ => {
        unimplemented!();
      }
    }
  }
  Ok(())
}

#[napi]
pub fn diff_sig(sig_source: String, dest: String) -> Result<()> {
  let sig_data = fs::read(sig_source).unwrap();
  let source_sig = Signature::from(&sig_data).unwrap();

  let mut dest_file = File::open(dest).unwrap();
  let dest_sig = Signature::new(&mut dest_file).unwrap();

  if source_sig.version != dest_sig.version {
    return Err(Error::from_reason(format!(
      "signature version mismatch: wants {}, found {}",
      dest_sig.version, source_sig.version
    )));
  }

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
  use std::io::Cursor;
  let data: Vec<u8> = (0..100500).map(|_| rand::random::<u8>()).collect();
  let mut buffer = Cursor::new(&data[..]);
  let sig = Signature::new(&mut buffer).unwrap();
  let mut serialized_data = Vec::new();
  sig.write(&mut serialized_data).unwrap();

  let sig_re = Signature::from(&serialized_data).unwrap();
  assert_eq!(sig, sig_re);
}

// #[napi]
// pub fn apply(diff: String, target: String) -> Result<()> {
//   Ok(())
// }

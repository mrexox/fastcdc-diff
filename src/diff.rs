use crate::signature::{Chunk, Signature};

use std::collections::HashMap;
use std::error::Error;
use std::io::{self, copy, Read, Seek, SeekFrom, Write};

#[derive(Debug)]
pub(crate) enum Op {
  Equal,
  Insert,
}

impl Op {
  fn to_u8(&self) -> u8 {
    match self {
      Op::Equal => 0,
      Op::Insert => 1,
    }
  }

  pub(crate) fn from_u8(op: u8) -> Self {
    match op {
      0 => Op::Equal,
      1 => Op::Insert,
      _ => unimplemented!(),
    }
  }
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
pub(crate) fn write_diff_between<R, W>(
  a: &Signature,
  b: &Signature,
  b_data: &mut R,
  dest: &mut W,
) -> Result<(), Box<dyn Error>>
where
  R: Read + Seek,
  W: Write,
{
  // Write the tool version
  dest.write_all(&[a.version])?;

  // Write the operations
  for op in diff_signatures(a, b).iter() {
    match op.0 {
      Op::Equal => {
        serialize_equal(op.1, op.2, dest)?;
      }
      Op::Insert => {
        serialize_insert(op.1, op.2, b_data, dest)?;
      }
    }
  }

  Ok(())
}

/// Returns a vector with tuples: (Op, offset, size).
/// For `Op::Insert` offset and size refer to the target file.
/// For `Op::Equal` offset and size refer to the source file.
pub(crate) fn diff_signatures<'a>(a: &'a Signature, b: &'a Signature) -> Vec<(Op, u64, u64)> {
  let mut original_chunks: HashMap<blake3::Hash, &Chunk> = HashMap::with_capacity(a.chunks.len());
  for chunk in a.chunks.iter() {
    original_chunks.entry(chunk.hash).or_insert(&chunk);
  }

  let mut diff: Vec<(Op, u64, u64)> = Vec::new();
  let mut current_op: Op = Op::Equal;
  let mut current_length = 0;
  let mut current_offset = 0;
  for new_chunk in b.chunks.iter() {
    match original_chunks.get(&new_chunk.hash) {
      Some(&chunk) => match current_op {
        Op::Equal => {
          if current_offset + current_length == chunk.offset {
            current_length += chunk.length as u64;
          } else {
            diff.push((Op::Equal, current_offset, current_length));
            current_offset = chunk.offset;
            current_length = chunk.length as u64;
          }
        }
        Op::Insert => {
          if current_length > 0 {
            diff.push((Op::Insert, current_offset, current_length));
            current_offset = chunk.offset;
          }
          current_length = chunk.length as u64;
          current_op = Op::Equal;
        }
      },
      None => match current_op {
        Op::Insert => {
          if current_offset + current_length == new_chunk.offset {
            current_length += new_chunk.length as u64;
          } else {
            diff.push((Op::Insert, current_offset, current_length));
            current_offset = new_chunk.offset;
            current_length = new_chunk.length as u64;
          }
        }
        Op::Equal => {
          if current_length > 0 {
            diff.push((Op::Equal, current_offset, current_length));
            current_offset = new_chunk.offset;
          }
          current_length = new_chunk.length as u64;
          current_op = Op::Insert;
        }
      },
    }
  }
  diff.push((current_op, current_offset, current_length));

  diff
}

pub(crate) fn serialize_insert<R, W>(
  offset: u64,
  size: u64,
  source: &mut R,
  dest: &mut W,
) -> Result<(), io::Error>
where
  R: Read + Seek,
  W: Write,
{
  dest.write_all(&[Op::Insert.to_u8()])?;
  dest.write_all(size.to_be_bytes().as_ref())?;

  source.seek(SeekFrom::Start(offset))?;
  let mut chunk = source.take(size);
  copy(&mut chunk, dest)?;

  Ok(())
}

pub(crate) fn serialize_equal<W: Write>(
  offset: u64,
  size: u64,
  dest: &mut W,
) -> Result<(), Box<dyn Error>> {
  dest.write_all(&[Op::Equal.to_u8()])?;
  dest.write_all(offset.to_be_bytes().as_ref())?;
  dest.write_all(size.to_be_bytes().as_ref())?;

  Ok(())
}

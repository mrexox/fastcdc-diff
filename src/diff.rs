use crate::signature::{Chunk, Signature};

use std::collections::HashMap;
use std::error::Error;
use std::io::{self, copy, Read, Seek, SeekFrom, Write};

/// Operation is an operation for applying the diff.
/// `Operation::Insert` is for inserting new data that is not present in the source file.
/// `Operation::Copy` is for copying existing data from the source file.
#[derive(Debug, PartialEq)]
pub(crate) enum Operation {
  Copy,
  Insert,
}

impl Operation {
  fn to_u8(&self) -> u8 {
    match self {
      Operation::Copy => 0,
      Operation::Insert => 1,
    }
  }

  pub(crate) fn from_u8(operation: u8) -> Self {
    match operation {
      0 => Operation::Copy,
      1 => Operation::Insert,
      _ => unimplemented!(),
    }
  }
}

/// Generate simple diff format:
///
/// VERSION(u8) - a diff file version for compatibility checking
/// OPERATION(u8) - 0/1, 0 means copy, 1 means insert
/// DATA:
///   for 0:
///     START OFFSET(u64) - offset of the file A to copy from
///     SIZE(u64) - size of a chunk to copy from A
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
      Operation::Copy => {
        serialize_copy(op.1, op.2, dest)?;
      }
      Operation::Insert => {
        serialize_insert(op.1, op.2, b_data, dest)?;
      }
    }
  }

  Ok(())
}

/// Returns a vector with tuples: (Operation, offset, size).
/// For `Operation::Insert` offset and size refer to the target file.
/// For `Operation::Copy` offset and size refer to the source file.
pub(crate) fn diff_signatures<'a>(
  a: &'a Signature,
  b: &'a Signature,
) -> Vec<(Operation, u64, u64)> {
  let mut original_chunks: HashMap<blake3::Hash, &Chunk> = HashMap::with_capacity(a.chunks.len());
  for chunk in a.chunks.iter() {
    original_chunks.entry(chunk.hash).or_insert(chunk);
  }

  let mut diff: Vec<(Operation, u64, u64)> = Vec::new();
  let mut current_op: Operation = Operation::Copy;
  let mut current_length = 0;
  let mut current_offset = 0;
  for new_chunk in b.chunks.iter() {
    match original_chunks.get(&new_chunk.hash) {
      Some(&chunk) => match current_op {
        Operation::Copy => {
          if current_offset + current_length == chunk.offset {
            current_length += chunk.length as u64;
          } else {
            if current_length > 0 {
              diff.push((Operation::Copy, current_offset, current_length));
            }
            current_offset = chunk.offset;
            current_length = chunk.length as u64;
          }
        }
        Operation::Insert => {
          if current_length > 0 {
            diff.push((Operation::Insert, current_offset, current_length));
            current_offset = chunk.offset;
          }
          current_length = chunk.length as u64;
          current_op = Operation::Copy;
        }
      },
      None => match current_op {
        Operation::Insert => {
          if current_offset + current_length == new_chunk.offset {
            current_length += new_chunk.length as u64;
          } else {
            if current_length > 0 {
              diff.push((Operation::Insert, current_offset, current_length));
            }
            current_offset = new_chunk.offset;
            current_length = new_chunk.length as u64;
          }
        }
        Operation::Copy => {
          if current_length > 0 {
            diff.push((Operation::Copy, current_offset, current_length));
            current_offset = new_chunk.offset;
          }
          current_length = new_chunk.length as u64;
          current_op = Operation::Insert;
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
  dest.write_all(&[Operation::Insert.to_u8()])?;
  dest.write_all(size.to_be_bytes().as_ref())?;

  source.seek(SeekFrom::Start(offset))?;
  let mut chunk = source.take(size);
  copy(&mut chunk, dest)?;

  Ok(())
}

pub(crate) fn serialize_copy<W: Write>(
  offset: u64,
  size: u64,
  dest: &mut W,
) -> Result<(), Box<dyn Error>> {
  dest.write_all(&[Operation::Copy.to_u8()])?;
  dest.write_all(offset.to_be_bytes().as_ref())?;
  dest.write_all(size.to_be_bytes().as_ref())?;

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::Chunk;
  use super::Operation;
  use super::Signature;

  #[test]
  fn test_diff_signatures() {
    let chunks1 = vec![
      Chunk {
        hash: [4u8; 32].into(),
        offset: 0,
        length: 16,
      },
      Chunk {
        hash: [0u8; 32].into(),
        offset: 16,
        length: 256,
      },
      Chunk {
        hash: [2u8; 32].into(),
        offset: 272,
        length: 18,
      },
    ];
    let sig1 = Signature {
      version: 0,
      min_size: 1024,
      avg_size: 1024,
      max_size: 2048,
      chunks: chunks1,
    };

    let chunks2 = vec![
      Chunk {
        hash: [0u8; 32].into(),
        offset: 0,
        length: 256,
      },
      Chunk {
        hash: [4u8; 32].into(),
        offset: 256,
        length: 16,
      },
      Chunk {
        hash: [5u8; 32].into(),
        offset: 272,
        length: 28,
      },
      Chunk {
        hash: [6u8; 32].into(),
        offset: 300,
        length: 12,
      },
      Chunk {
        hash: [2u8; 32].into(),
        offset: 312,
        length: 18,
      },
      Chunk {
        hash: [17u8; 32].into(),
        offset: 330,
        length: 10,
      },
    ];
    let sig2 = super::Signature {
      version: 0,
      min_size: 1024,
      avg_size: 1024,
      max_size: 2048,
      chunks: chunks2,
    };

    let res = super::diff_signatures(&sig1, &sig2);
    assert_eq!(
      res,
      vec![
        (Operation::Copy, 16, 256),
        (Operation::Copy, 0, 16),
        (Operation::Insert, 272, 40),
        (Operation::Copy, 272, 18),
        (Operation::Insert, 330, 10),
      ]
    )
  }
}

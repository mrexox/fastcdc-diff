use crate::signature::{Chunk, Signature};

use similar::utils::diff_slices;
use similar::{Algorithm, ChangeTag};
use std::error::Error;
use std::io::{self, copy, Read, Seek, SeekFrom, Write};

pub(crate) enum Op {
  Equal,
  Insert,
}

impl Op {
  fn to_u8(self) -> u8 {
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
      ChangeTag::Equal => {
        let start = &op.1[0];
        let end = &op.1[op.1.len() - 1];

        serialize_equal(start, end, dest)?;
      }
      ChangeTag::Insert => {
        let start = &op.1[0];
        let end = &op.1[op.1.len() - 1];

        serialize_insert(start, end, b_data, dest)?;
      }
      ChangeTag::Delete => {}
    }
  }

  Ok(())
}

pub(crate) fn diff_signatures<'a>(
  a: &'a Signature,
  b: &'a Signature,
) -> Vec<(ChangeTag, &'a [Chunk])> {
  diff_slices(Algorithm::Myers, &a.chunks, &b.chunks)
}

pub(crate) fn serialize_insert<R, W>(
  start: &Chunk,
  end: &Chunk,
  source: &mut R,
  dest: &mut W,
) -> Result<(), io::Error>
where
  R: Read + Seek,
  W: Write,
{
  dest.write_all(&[Op::Insert.to_u8()])?;

  let size = end.offset + end.length as u64 - start.offset;
  dest.write_all(size.to_be_bytes().as_ref())?;

  source.seek(SeekFrom::Start(start.offset))?;
  let mut chunk = source.take(size);
  copy(&mut chunk, dest)?;

  Ok(())
}

pub(crate) fn serialize_equal<W: Write>(
  start: &Chunk,
  end: &Chunk,
  dest: &mut W,
) -> Result<(), Box<dyn Error>> {
  dest.write_all(&[Op::Equal.to_u8()])?;
  dest.write_all(start.offset.to_be_bytes().as_ref())?;
  dest.write_all((end.offset + end.length as u64).to_be_bytes().as_ref())?;

  Ok(())
}

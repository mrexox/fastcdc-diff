use crate::diff::Op;
use crate::signature::VERSION;

use std::error::Error;
use std::fmt;
use std::io::{copy, ErrorKind, Read, Seek, SeekFrom, Write};

#[derive(Debug)]
struct VersionMismatch(u8);

impl fmt::Display for VersionMismatch {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "version mismatch: got {}, want {}", self.0, VERSION)
  }
}

impl Error for VersionMismatch {
  #[allow(deprecated, deprecated_in_future)]
  fn description(&self) -> &str {
    "versions mismatch"
  }
}

pub(crate) fn apply<R, W>(diff: &mut R, target: &mut R, dest: &mut W) -> Result<(), Box<dyn Error>>
where
  R: Read + Seek,
  W: Write,
{
  let mut buf: [u8; 1] = [0; 1];

  diff.read_exact(&mut buf)?;

  if buf[0] != VERSION {
    return Err(Box::new(VersionMismatch(buf[0])));
  }

  let mut u64buf: [u8; 8] = [0; 8];

  loop {
    if let Err(err) = diff.read_exact(&mut buf) {
      if err.kind() == ErrorKind::UnexpectedEof {
        break;
      }

      return Err(Box::new(err));
    }

    match Op::from_u8(buf[0]) {
      Op::Equal => {
        diff.read_exact(&mut u64buf)?;
        let offset = u64::from_be_bytes(u64buf);
        diff.read_exact(&mut u64buf)?;
        let size = u64::from_be_bytes(u64buf);

        target.seek(SeekFrom::Start(offset))?;
        let mut chunk = target.take(size);
        copy(&mut chunk, dest)?;
      }
      Op::Insert => {
        diff.read_exact(&mut u64buf)?;
        let size = u64::from_be_bytes(u64buf);
        let mut chunk = diff.take(size as u64);
        copy(&mut chunk, dest)?;
      }
    }
  }

  Ok(())
}

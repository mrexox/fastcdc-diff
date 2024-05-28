use crate::diff::Operation;
use crate::signature::VERSION;

use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, RANGE};
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

pub(crate) fn apply<R, W>(diff: &mut R, source: &mut R, dest: &mut W) -> Result<(), Box<dyn Error>>
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

    match Operation::from_u8(buf[0]) {
      Operation::Copy => {
        diff.read_exact(&mut u64buf)?;
        let offset = u64::from_be_bytes(u64buf);
        diff.read_exact(&mut u64buf)?;
        let size = u64::from_be_bytes(u64buf);

        source.seek(SeekFrom::Start(offset))?;
        let mut chunk = source.take(size);
        copy(&mut chunk, dest)?;
      }
      Operation::Insert => {
        diff.read_exact(&mut u64buf)?;
        let size = u64::from_be_bytes(u64buf);
        let mut chunk = diff.take(size);
        copy(&mut chunk, dest)?;
      }
    }
  }

  Ok(())
}

/// Downloads missing diff chunks, stores them in a temporary file and uses them along with `source`
/// to construct the new file.
pub(crate) fn apply_from_http<R, W>(
  diff: Vec<(Operation, u64, u64)>,
  uri: String,
  source: &mut R,
  dest: &mut W,
) -> Result<(), Box<dyn Error>>
where
  R: Read + Seek,
  W: Write,
{
  let client = Client::new();
  let remote_data = &mut tempfile::tempfile()?;

  for d in diff.iter() {
    match d.0 {
      Operation::Copy => {}
      Operation::Insert => {
        let mut headers = HeaderMap::new();
        headers.insert(
          RANGE,
          HeaderValue::from_str(format!("bytes={}-{}", d.1, d.1 + d.2 - 1).as_ref())?,
        );
        let mut response = client.get(&uri).headers(headers).send()?;
        let _ = response.copy_to(remote_data);
      }
    }
  }

  remote_data.seek(SeekFrom::Start(0))?;

  for d in diff.iter() {
    match d.0 {
      Operation::Copy => {
        source.seek(SeekFrom::Start(d.1))?;
        let mut chunk = source.take(d.2);
        copy(&mut chunk, dest)?;
      }
      Operation::Insert => {
        let mut chunk = remote_data.take(d.2);
        copy(&mut chunk, dest)?;
      }
    }
  }

  Ok(())
}

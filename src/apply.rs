use crate::diff::Operation;
use crate::signature::VERSION;

use reqwest::header::RANGE;
use reqwest::Client;
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

    match buf[0].into() {
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
pub(crate) async fn apply_from_http<R, W>(
  diff: Vec<(Operation, u64, u64)>,
  uri: String,
  source: &mut R,
  dest: &mut W,
) -> Result<(), Box<dyn Error>>
where
  R: Read + Seek,
  W: Write,
{
  let remote_data = &mut tempfile::tempfile()?;
  let mut byte_ranges = Vec::new();

  for d in diff.iter() {
    match d.0 {
      Operation::Copy => {}
      Operation::Insert => {
        byte_ranges.push((d.1, d.1 + d.2 - 1));
      }
    }
  }

  let mut tasks = Vec::with_capacity(byte_ranges.len());
  for (start, end) in byte_ranges {
    let url = uri.clone();
    let task = napi::tokio::task::spawn(async move {
      Client::new()
        .get(url)
        .header(RANGE, format!("bytes={}-{}", start, end))
        .send()
        .await
    });
    tasks.push(task);
  }

  for task in tasks {
    let mut response = task.await??;
    while let Some(chunk) = response.chunk().await? {
      remote_data.write_all(&chunk)?;
    }
  }

  remote_data.seek(SeekFrom::Start(0))?;

  for (op, offset, size) in diff {
    match op {
      Operation::Copy => {
        source.seek(SeekFrom::Start(offset))?;
        let mut chunk = source.take(size);
        copy(&mut chunk, dest)?;
      }
      Operation::Insert => {
        let mut chunk = remote_data.take(size);
        copy(&mut chunk, dest)?;
      }
    }
  }

  Ok(())
}

#![deny(clippy::all)]

mod apply;
mod diff;
mod signature;

use napi::bindgen_prelude::*;
use std::default::Default;
use std::fs::File;
use std::path::Path;

use crate::signature::Signature;

#[macro_use]
extern crate napi_derive;

#[napi(object)]
pub struct SignatureOptions {
  pub min_size: u32,
  pub avg_size: u32,
  pub max_size: u32,
}

impl Default for SignatureOptions {
  fn default() -> Self {
    SignatureOptions {
      min_size: signature::DEFAULT_MIN_SIZE,
      avg_size: signature::DEFAULT_AVG_SIZE,
      max_size: signature::DEFAULT_MAX_SIZE,
    }
  }
}

/// Writes calculated signature for `source` to the `dest`.
#[napi]
pub fn signature_to_file(
  source: String,
  dest: String,
  options: Option<SignatureOptions>,
) -> Result<()> {
  if !Path::new(&source).exists() {
    return Err(Error::from_reason(format!("file {source} does not exist")));
  }

  let mut source = File::open(source).unwrap();
  let mut dest = File::create(dest).unwrap();

  let options = options.unwrap_or(SignatureOptions::default());

  let signature = Signature::calculate(
    &mut source,
    options.min_size,
    options.avg_size,
    options.max_size,
  )
  .unwrap();
  signature.write(&mut dest).unwrap();

  Ok(())
}

/// Returns calculated signature of the `source`.
#[napi]
pub fn signature(source: String, options: Option<SignatureOptions>) -> Result<Buffer> {
  if !Path::new(&source).exists() {
    return Err(Error::from_reason(format!("file {source} does not exist")));
  }

  let options = options.unwrap_or(SignatureOptions::default());

  let mut source = File::open(source).unwrap();
  let signature = Signature::calculate(
    &mut source,
    options.min_size,
    options.avg_size,
    options.max_size,
  )
  .unwrap();

  let mut dest = Vec::new();
  signature.write(&mut dest).unwrap();

  Ok(dest.into())
}

/// Writes a diff that transforms `a` -> `b` into `dest`.
#[napi]
pub fn diff(a: String, b: String, dest: String, options: Option<SignatureOptions>) -> Result<()> {
  let options = options.unwrap_or(SignatureOptions::default());

  let mut a_file = File::open(a).unwrap();
  let a_sig = Signature::calculate(
    &mut a_file,
    options.min_size,
    options.avg_size,
    options.max_size,
  )
  .unwrap();

  let mut b_file = File::open(b).unwrap();
  let b_sig = Signature::calculate(
    &mut b_file,
    options.min_size,
    options.avg_size,
    options.max_size,
  )
  .unwrap();

  let mut dest_file = File::create(dest).unwrap();

  diff::write_diff_between(&a_sig, &b_sig, &mut b_file, &mut dest_file).unwrap();

  Ok(())
}

/// Applies `diff_path` to the `a` and writes the result to `result`.
#[napi]
pub fn apply(diff_path: String, a: String, result: String) -> Result<()> {
  let mut diff_file = File::open(diff_path).unwrap();

  let mut target_file = File::open(a).unwrap();
  let mut res_file = File::create(result).unwrap();

  apply::apply(&mut diff_file, &mut target_file, &mut res_file).unwrap();

  Ok(())
}

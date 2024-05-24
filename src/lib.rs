#![deny(clippy::all)]

mod apply;
mod diff;
mod signature;

use napi::bindgen_prelude::*;
use std::default::Default;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use crate::signature::Signature;
use cloud_zsync::builder::{build_local_diff_file, build_local_file};
use cloud_zsync::signature::{Diff as CloudDiff, Signature as CloudSignature};

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
pub fn write_binary_signature(
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

#[napi]
pub fn write_cloud_signature(
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

  let signature = CloudSignature::generate(
    &mut source,
    options.min_size,
    options.avg_size,
    options.max_size,
  )
  .unwrap();

  let serialized = serde_json::to_string_pretty(&signature).unwrap();
  dest.write_all(serialized.as_bytes()).unwrap();

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

/// Calculates diff based on source file signature and the target file.
#[napi]
pub fn signature_diff(sig: String, target: String, dest: String) -> Result<()> {
  let sig_data = fs::read(sig).unwrap();
  let signature = Signature::load(&sig_data);

  let mut target_file = File::open(target).unwrap();
  let target_signature = Signature::calculate(
    &mut target_file,
    signature.min_size,
    signature.avg_size,
    signature.max_size,
  )
  .unwrap();

  let mut dest_file = File::create(dest).unwrap();

  diff::write_diff_between(
    &signature,
    &target_signature,
    &mut target_file,
    &mut dest_file,
  )
  .unwrap();

  Ok(())
}

// TODO: Implement with fetching data from the Cloud
// #[napi]
// pub fn cloud_signature_diff(sig: String, target: String, dest: String) -> Result<()> {
//   let sig_file = File::open(sig).unwrap();
//   let signature: CloudSignature = serde_json::from_reader(sig_file).unwrap();

//   let mut target_file = File::open(target).unwrap();
//   let target_sig = CloudSignature::generate(
//     &mut target_file,
//     // TODO: this must be taken from the deserialized signature
//     signature::DEFAULT_MIN_SIZE,
//     signature::DEFAULT_AVG_SIZE,
//     signature::DEFAULT_MAX_SIZE,
//   )
//   .unwrap();

//   let diff = match CloudDiff::new(&signature, &target_sig) {
//     Some(diff) => diff,
//     None => return Err(Error::from_reason("files are equal")),
//   };

//   // let mut dest_file = File::create(dest).unwrap();
//   // let diff_schema =
//   //   build_local_diff_file(&mut target_file, &mut dest_file, diff.operations().iter());

//   // // build_local_file();

//   Ok(())
// }

/// Applies `diff` to the `a` and writes the result to `result`.
#[napi]
pub fn apply(diff: String, a: String, result: String) -> Result<()> {
  let mut diff_file = File::open(diff).unwrap();

  let mut target_file = File::open(a).unwrap();
  let mut res_file = File::create(result).unwrap();

  apply::apply(&mut diff_file, &mut target_file, &mut res_file).unwrap();

  Ok(())
}

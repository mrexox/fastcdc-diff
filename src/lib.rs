#![deny(clippy::all)]

mod apply;
mod diff;
mod signature;

use anyhow::Context;
use futures::prelude::*;
use napi::bindgen_prelude::*;
use std::default::Default;
use std::fs::{self, File};

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
pub fn write_binary_signature(
  source: String,
  dest: String,
  options: Option<SignatureOptions>,
) -> Result<()> {
  let mut source_file = open_file(&source)?;
  let mut dest = create_file(&dest)?;
  let options = options.unwrap_or_default();

  let signature = Signature::calculate(
    &mut source_file,
    options.min_size,
    options.avg_size,
    options.max_size,
  )
  .with_context(|| format!("Failed to calculate the sugnagure for {}", &source))
  .map_err(anyhow_to_js_error)?;
  signature
    .write(&mut dest)
    .context("Failed to write the signature to the file")
    .map_err(anyhow_to_js_error)?;

  Ok(())
}

/// Returns calculated signature of the `source`.
#[napi]
pub fn signature(source: String, options: Option<SignatureOptions>) -> Result<Buffer> {
  let options = options.unwrap_or_default();

  let mut source_file = open_file(&source)?;
  let signature = Signature::calculate(
    &mut source_file,
    options.min_size,
    options.avg_size,
    options.max_size,
  )
  .with_context(|| format!("Failed to calculate the sugnagure for {}", &source))
  .map_err(anyhow_to_js_error)?;

  let mut dest = Vec::new();
  signature.write(&mut dest).map_err(to_js_error)?;

  Ok(dest.into())
}

/// Generates a diff that transforms `source` to `target`.
#[napi]
pub fn diff(
  source: String,
  target: String,
  dest: String,
  options: Option<SignatureOptions>,
) -> Result<()> {
  let options = options.unwrap_or_default();

  let mut source_file = open_file(&source)?;
  let source_signature = Signature::calculate(
    &mut source_file,
    options.min_size,
    options.avg_size,
    options.max_size,
  )
  .with_context(|| format!("Failed to calculate the sugnagure for {}", &source))
  .map_err(anyhow_to_js_error)?;

  let mut target_file = open_file(&target)?;
  let target_signature = Signature::calculate(
    &mut target_file,
    options.min_size,
    options.avg_size,
    options.max_size,
  )
  .with_context(|| format!("Failed to calculate the sugnagure for {}", &target))
  .map_err(anyhow_to_js_error)?;

  let mut dest_file = create_file(&dest)?;

  diff::write_diff_between(
    &source_signature,
    &target_signature,
    &mut target_file,
    &mut dest_file,
  )
  .map_err(box_to_js_error)?;

  Ok(())
}

/// Generates a diff that transforms `source` to `target. Only source signature is required.
#[napi]
pub fn diff_using_source_signature(source_sig: String, target: String, dest: String) -> Result<()> {
  let sig_data = fs::read(source_sig).map_err(to_js_error)?;
  let source_signature = Signature::load(&sig_data);

  let mut target_file = open_file(&target)?;
  let target_signature = Signature::calculate(
    &mut target_file,
    source_signature.min_size,
    source_signature.avg_size,
    source_signature.max_size,
  )
  .with_context(|| format!("Failed to calculate the sugnagure for {}", &target))
  .map_err(anyhow_to_js_error)?;

  let mut dest_file = create_file(&dest)?;

  diff::write_diff_between(
    &source_signature,
    &target_signature,
    &mut target_file,
    &mut dest_file,
  )
  .map_err(box_to_js_error)?;

  Ok(())
}

/// Downloads the required parts of the file and builds a new file based on `target_sig` and the
/// `source`.
#[napi]
pub async fn pull_using_remote_signature(
  source: String,
  target_sig: String,
  file_uri: String,
  dest: String,
) -> Result<()> {
  let sig_data = fs::read(target_sig).map_err(to_js_error)?;
  let target_signature = Signature::load(&sig_data);

  let mut source_file = open_file(&source)?;
  let source_signature = Signature::calculate(
    &mut source_file,
    target_signature.min_size,
    target_signature.avg_size,
    target_signature.max_size,
  )
  .with_context(|| format!("Failed to calculate the sugnagure for {}", &source))
  .map_err(anyhow_to_js_error)?;

  let sig_diff = diff::diff_signatures(&source_signature, &target_signature);

  let mut dest_file = create_file(&dest)?;
  apply::apply_from_http(sig_diff, file_uri, &mut source_file, &mut dest_file)
    .map_err(box_to_js_error)
    .await?;

  Ok(())
}

/// Applies `diff` to the `a` and writes the result to `result`.
#[napi]
pub fn apply(diff: String, a: String, result: String) -> Result<()> {
  let mut diff_file = open_file(&diff)?;
  let mut target_file = open_file(&a)?;
  let mut res_file = File::create(result).map_err(to_js_error)?;

  apply::apply(&mut diff_file, &mut target_file, &mut res_file).map_err(box_to_js_error)?;

  Ok(())
}

fn open_file(path: &str) -> Result<File> {
  File::open(path)
    .with_context(|| format!("Failed to open a file {}", path))
    .map_err(anyhow_to_js_error)
}

fn create_file(path: &str) -> Result<File> {
  File::create(path)
    .with_context(|| format!("Failed to create a file {}", path))
    .map_err(anyhow_to_js_error)
}

fn to_js_error(e: impl std::error::Error) -> Error {
  Error::from_reason(e.to_string())
}

fn anyhow_to_js_error(e: anyhow::Error) -> Error {
  Error::from_reason(e.to_string())
}

fn box_to_js_error(e: Box<dyn std::error::Error>) -> Error {
  Error::from_reason(e.to_string())
}

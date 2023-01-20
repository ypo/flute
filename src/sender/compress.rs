use std::io::Write;

use crate::common::lct;
use crate::tools::error::{FluteError, Result};
use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};

pub fn compress(data: &[u8], cenc: lct::Cenc) -> Result<Vec<u8>> {
    match cenc {
        lct::Cenc::Null => Err(FluteError::new("Null compression ?")),
        lct::Cenc::Zlib => compress_zlib(data),
        lct::Cenc::Deflate => compress_deflate(data),
        lct::Cenc::Gzip => compress_gzip(data),
    }
}

fn compress_gzip(data: &[u8]) -> Result<Vec<u8>> {
    log::debug!("Create GZIP encoder");
    let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data)?;
    let output = encoder.finish()?;
    Ok(output)
}

fn compress_deflate(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data)?;
    let output = encoder.finish()?;
    Ok(output)
}

fn compress_zlib(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data)?;
    let output = encoder.finish()?;
    Ok(output)
}

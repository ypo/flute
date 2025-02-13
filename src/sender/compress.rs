//! This module provides functions for compressing data using various compression algorithms.
//!
//! The supported compression algorithms are:
//! - Zlib
//! - Deflate
//! - Gzip
//!
//! The module offers two main functions for compression:
//! - `compress_buffer`: Compresses a byte slice and returns the compressed data as a vector of bytes.
//! - `compress_stream`: Compresses data from an input stream and writes the compressed data to an output stream.
//!
//! # Examples
//!
//! ## Compressing a Byte Slice
//!
//! ```rust
//! use flute::sender::compress::compress_buffer;
//! use flute::core::lct::Cenc;
//!
//! let data = b"example data";
//! let compressed_data = compress_buffer(data, Cenc::Gzip).expect("Compression failed");
//! ```
//!
//! ## Compressing Data from a Stream
//!
//! ```rust
//! use flute::sender::compress::compress_stream;
//! use flute::core::lct::Cenc;
//! use std::io::Cursor;
//!
//! let data = b"example data";
//! let mut input = Cursor::new(data);
//! let mut output = Vec::new();
//! compress_stream(&mut input, Cenc::Gzip, &mut output).expect("Compression failed");
//! ```

use crate::common::lct;
use crate::tools::error::{FluteError, Result};
use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use std::io::Write;

/// Compresses the given data using the specified compression encoding.
///
/// # Arguments
///
/// * `data` - A byte slice containing the data to be compressed.
/// * `cenc` - The compression encoding to use. This can be one of the following:
///     - `lct::Cenc::Null`: No compression (returns an error).
///     - `lct::Cenc::Zlib`: Compress using the Zlib algorithm.
///     - `lct::Cenc::Deflate`: Compress using the Deflate algorithm.
///     - `lct::Cenc::Gzip`: Compress using the Gzip algorithm.
///
/// # Returns
///
/// A `Result` containing a vector of compressed bytes on success, or a `FluteError` on failure.
///
/// # Errors
///
/// This function will return an error if the specified compression encoding is `lct::Cenc::Null`
/// or if there is an issue during the compression process.
///
/// # Examples
///
/// ```
/// use flute::sender::compress::compress_buffer;
/// use flute::core::lct::Cenc;
///
/// let data = b"example data";
/// let compressed_data = compress_buffer(data, Cenc::Gzip).expect("Compression failed");
/// ```
pub fn compress_buffer(data: &[u8], cenc: lct::Cenc) -> Result<Vec<u8>> {
    match cenc {
        lct::Cenc::Null => Err(FluteError::new("Null compression ?")),
        lct::Cenc::Zlib => compress_zlib(data),
        lct::Cenc::Deflate => compress_deflate(data),
        lct::Cenc::Gzip => compress_gzip(data),
    }
}

/// Compresses data from an input stream and writes the compressed data to an output stream
/// using the specified compression encoding.
///
/// # Arguments
///
/// * `input` - A mutable reference to a type that implements the `std::io::Read` trait, representing the input stream.
/// * `cenc` - The compression encoding to use. This can be one of the following:
///     - `lct::Cenc::Null`: No compression (returns an error).
///     - `lct::Cenc::Zlib`: Compress using the Zlib algorithm.
///     - `lct::Cenc::Deflate`: Compress using the Deflate algorithm.
///     - `lct::Cenc::Gzip`: Compress using the Gzip algorithm.
/// * `output` - A mutable reference to a type that implements the `std::io::Write` trait, representing the output stream.
///
/// # Returns
///
/// A `Result` containing `()` on success, or a `FluteError` on failure.
///
/// # Errors
///
/// This function will return an error if the specified compression encoding is `lct::Cenc::Null`
/// or if there is an issue during the compression process.
/// ```
pub fn compress_stream(
    input: &mut dyn std::io::Read,
    cenc: lct::Cenc,
    output: &mut dyn std::io::Write,
) -> Result<()> {
    match cenc {
        lct::Cenc::Null => Err(FluteError::new("Null compression ?")),
        lct::Cenc::Zlib => stream_compress_zlib(input, output),
        lct::Cenc::Deflate => stream_compress_deflate(input, output),
        lct::Cenc::Gzip => stream_compress_gzip(input, output),
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

fn stream_compress_gzip(
    input: &mut dyn std::io::Read,
    output: &mut dyn std::io::Write,
) -> Result<()> {
    log::debug!("Create GZIP encoder");
    let mut encoder = GzEncoder::new(output, flate2::Compression::default());
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        encoder.write_all(&buffer[..read])?;
    }

    encoder.finish()?;
    Ok(())
}

fn stream_compress_zlib(
    input: &mut dyn std::io::Read,
    output: &mut dyn std::io::Write,
) -> Result<()> {
    log::debug!("Create GZIP encoder");
    let mut encoder = ZlibEncoder::new(output, flate2::Compression::default());
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        encoder.write_all(&buffer[..read])?;
    }

    encoder.finish()?;
    Ok(())
}

fn stream_compress_deflate(
    input: &mut dyn std::io::Read,
    output: &mut dyn std::io::Write,
) -> Result<()> {
    log::debug!("Create GZIP encoder");
    let mut encoder = DeflateEncoder::new(output, flate2::Compression::default());
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        encoder.write_all(&buffer[..read])?;
    }

    encoder.finish()?;
    Ok(())
}

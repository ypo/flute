use std::time::SystemTime;

use base64::Engine;

use crate::error::{FluteError, Result};

use crate::common::lct;

use super::{
    blockdecoder::BlockDecoder,
    uncompress::{Decompress, DecompressGzip},
    uncompress::{DecompressDeflate, DecompressZlib},
    writer::ObjectWriter,
};

pub struct BlockWriter {
    sbn: u32,
    bytes_left: usize,
    content_length_left: Option<usize>,
    cenc: lct::Cenc,
    decoder: Option<Box<dyn Decompress>>,
    buffer: Vec<u8>,
    md5_context: Option<md5::Context>,
    md5: Option<String>,
}

impl std::fmt::Debug for BlockWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockWriter")
            .field("sbn", &self.sbn)
            .field("bytes_left", &self.bytes_left)
            .field("cenc", &self.cenc)
            .field("decoder", &self.decoder)
            .field("buffer", &self.buffer)
            .field("md5_context", &self.md5_context.is_some())
            .field("md5", &self.md5)
            .finish()
    }
}

impl BlockWriter {
    pub fn new(
        transfer_length: usize,
        content_length: Option<usize>,
        cenc: lct::Cenc,
        md5: bool,
    ) -> BlockWriter {
        BlockWriter {
            sbn: 0,
            bytes_left: transfer_length,
            content_length_left: content_length,
            cenc,
            decoder: None,
            buffer: Vec::new(),
            md5_context: match md5 {
                true => Some(md5::Context::new()),
                false => None,
            },
            md5: None,
        }
    }

    pub fn check_md5(&self, md5: &str) -> bool {
        self.md5.as_ref().map(|m| m.eq(md5)).unwrap_or(true)
    }

    pub fn get_md5(&self) -> Option<&str> {
        self.md5.as_deref()
    }

    pub fn write(
        &mut self,
        sbn: u32,
        block: &BlockDecoder,
        writer: &dyn ObjectWriter,
        now: SystemTime,
    ) -> Result<bool> {
        if self.sbn != sbn {
            return Ok(false);
        }
        debug_assert!(block.completed);
        let data = block.source_block()?;

        // Detect the size of the last symbol
        let data = match self.bytes_left > data.len() {
            true => data,
            false => &data[..self.bytes_left],
        };

        if self.cenc == lct::Cenc::Null {
            self.write_pkt_cenc_null(data, writer, now)?;
        } else {
            self.decode_write_pkt(data, writer, now)?;
        }

        debug_assert!(data.len() <= self.bytes_left);
        self.bytes_left -= data.len();

        self.sbn += 1;

        if self.is_completed() {
            // All blocks have been received -> flush the decoder
            if self.decoder.is_some() {
                self.decoder.as_mut().unwrap().finish();
                self.decoder_read(writer, now)?;
            }

            let output = self.md5_context.take().map(|ctx| ctx.compute().0);
            self.md5 =
                output.map(|output| base64::engine::general_purpose::STANDARD.encode(output));
        }

        Ok(true)
    }

    fn init_decoder(&mut self, data: &[u8]) {
        debug_assert!(self.decoder.is_none());
        self.decoder = match self.cenc {
            lct::Cenc::Null => None,
            lct::Cenc::Zlib => Some(Box::new(DecompressZlib::new(data))),
            lct::Cenc::Deflate => Some(Box::new(DecompressDeflate::new(data))),
            lct::Cenc::Gzip => Some(Box::new(DecompressGzip::new(data))),
        };
        self.buffer.resize(data.len(), 0);
    }

    fn write_pkt_cenc_null(&mut self, data: &[u8], writer: &dyn ObjectWriter, now: SystemTime) -> Result<()> {
        if let Some(ctx) = self.md5_context.as_mut() {
            ctx.consume(data)
        }
        writer.write(self.sbn, data, now)
    }

    fn decode_write_pkt(
        &mut self,
        pkt: &[u8],
        writer: &dyn ObjectWriter,
        now: SystemTime,
    ) -> Result<()> {
        if self.decoder.is_none() {
            self.init_decoder(pkt);
            self.decoder_read(writer, now)?;
            return Ok(());
        }

        let mut offset: usize = 0;
        loop {
            let size = self.decoder.as_mut().unwrap().write(&pkt[offset..])?;
            self.decoder_read(writer, now)?;
            offset += size;
            if offset == pkt.len() {
                break;
            }
        }
        Ok(())
    }

    fn decoder_read(&mut self, writer: &dyn ObjectWriter, now: SystemTime) -> Result<()> {
        let decoder = self.decoder.as_mut().unwrap();

        if self.content_length_left == Some(0) {
            return Ok(());
        }

        loop {
            let size = match decoder.read(&mut self.buffer) {
                Ok(res) => res,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) => return Err(FluteError::new(e)),
            };

            if size == 0 {
                return Ok(());
            }

            if let Some(ctx) = self.md5_context.as_mut() {
                ctx.consume(&self.buffer[..size])
            }

            writer.write(self.sbn, &self.buffer[..size], now)?;

            if let Some(content_length_left) = self.content_length_left.as_mut() {
                *content_length_left = content_length_left.saturating_sub(size);
                if *content_length_left == 0 {
                    return Ok(());
                }
            }
        }
    }

    pub fn left(&self) -> usize {
        self.bytes_left
    }

    pub fn is_completed(&self) -> bool {
        self.bytes_left == 0
    }
}

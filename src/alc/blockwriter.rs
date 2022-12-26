use crate::{
    alc::uncompress::{DecompressDeflate, DecompressZlib},
    error::{FluteError, Result},
};

use super::{
    blockdecoder::BlockDecoder,
    lct,
    objectwriter::ObjectWriterSession,
    uncompress::{Decompress, DecompressGzip},
};

pub struct BlockWriter {
    snb: u32,
    bytes_left: usize,
    cenc: lct::CENC,
    decoder: Option<Box<dyn Decompress>>,
    buffer: Vec<u8>,
    md5_context: Option<md5::Context>,
    md5: Option<String>,
}

impl std::fmt::Debug for BlockWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockWriter")
            .field("snb", &self.snb)
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
    pub fn new(transfer_length: usize, cenc: lct::CENC, md5: bool) -> BlockWriter {
        BlockWriter {
            snb: 0,
            bytes_left: transfer_length,
            cenc: cenc,
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
        log::info!("Check MD5 {} {:?}", md5, self.md5);
        self.md5.as_ref().map(|m| m.eq(md5)).unwrap_or(true)
    }

    pub fn write(
        &mut self,
        snb: u32,
        block: &BlockDecoder,
        writer: &dyn ObjectWriterSession,
    ) -> Result<bool> {
        if self.snb != snb {
            return Ok(false);
        }
        assert!(block.completed);
        let data = block.source_encoding_symbols();
        for encoding_symbol in data {
            // Block is completed , source symbol must not be None
            let symbols = encoding_symbol.as_ref().unwrap();

            // Detect the size of the last symbol
            let symbols = match self.bytes_left > symbols.len() {
                true => symbols.as_ref(),
                false => &symbols[..self.bytes_left],
            };

            if self.cenc == lct::CENC::Null {
                self.write_pkt_cenc_null(symbols, writer);
            } else {
                self.decode_write_pkt(symbols, writer)?;
            }

            assert!(symbols.len() <= self.bytes_left);
            self.bytes_left -= symbols.len();
        }

        if self.is_completed() {
            // All blocks have been received -> flush the decoder
            if self.decoder.is_some() {
                self.decoder.as_mut().unwrap().finish();
                self.decoder_read(writer)?;
            }

            self.md5 = self
                .md5_context
                .take()
                .map(|ctx| base64::encode(ctx.compute().0));
        }

        Ok(true)
    }

    fn init_decoder(&mut self, data: &[u8]) {
        assert!(self.decoder.is_none());
        self.decoder = match self.cenc {
            lct::CENC::Null => None,
            lct::CENC::Zlib => Some(Box::new(DecompressZlib::new(data))),
            lct::CENC::Deflate => Some(Box::new(DecompressDeflate::new(data))),
            lct::CENC::Gzip => Some(Box::new(DecompressGzip::new(data))),
        };
        self.buffer.resize(data.len(), 0);
    }

    fn write_pkt_cenc_null(&mut self, data: &[u8], writer: &dyn ObjectWriterSession) {
        self.md5_context.as_mut().map(|ctx| ctx.consume(data));
        writer.write(data);
    }

    fn decode_write_pkt(&mut self, pkt: &[u8], writer: &dyn ObjectWriterSession) -> Result<()> {
        if self.decoder.is_none() {
            self.init_decoder(pkt);
            self.decoder_read(writer)?;
            return Ok(());
        }

        let mut offset: usize = 0;
        loop {
            let size = self.decoder.as_mut().unwrap().write(&pkt[offset..])?;
            self.decoder_read(writer)?;
            offset += size;
            if offset == pkt.len() {
                break;
            }
        }
        Ok(())
    }

    fn decoder_read(&mut self, writer: &dyn ObjectWriterSession) -> Result<()> {
        let decoder = self.decoder.as_mut().unwrap();
        loop {
            let size = match decoder.read(&mut self.buffer) {
                Ok(res) => res,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) => return Err(FluteError::new(e)),
            };

            if size == 0 {
                return Ok(());
            }

            self.md5_context
                .as_mut()
                .map(|ctx| ctx.consume(&self.buffer[..size]));
            writer.write(&self.buffer[..size]);
        }
    }

    pub fn is_completed(&self) -> bool {
        self.bytes_left == 0
    }
}

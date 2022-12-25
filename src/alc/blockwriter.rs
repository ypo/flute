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

#[derive(Debug)]
pub struct BlockWriter {
    snb: u32,
    bytes_left: usize,
    cenc: lct::CENC,
    decoder: Option<Box<dyn Decompress>>,
    buffer: Vec<u8>,
}

impl BlockWriter {
    pub fn new(transfer_length: usize, cenc: lct::CENC) -> BlockWriter {
        BlockWriter {
            snb: 0,
            bytes_left: transfer_length,
            cenc: cenc,
            decoder: None,
            buffer: Vec::new(),
        }
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
            let symbols = encoding_symbol.as_ref().unwrap();
            let symbols = match self.bytes_left > symbols.len() {
                true => symbols.as_ref(),
                false => &symbols[..self.bytes_left],
            };
            if self.cenc == lct::CENC::Null {
                writer.write(symbols);
            } else {
                self.decode_write_pkt(symbols, writer)?;
            }
            assert!(symbols.len() <= self.bytes_left);
            self.bytes_left -= symbols.len();
        }

        if self.decoder.is_some() {
            self.decoder_read(writer)?;
        }

        if self.is_completed() {
            self.finish(writer)?;
        }

        Ok(true)
    }

    fn finish(&mut self, writer: &dyn ObjectWriterSession) -> Result<()> {
        if self.decoder.is_some() {
            self.decoder.as_mut().unwrap().finish();
            self.decoder_read(writer)?;
        }
        Ok(())
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
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    return Ok(());
                }
                Err(e) => return Err(FluteError::new(e)),
            };

            if size == 0 {
                return Ok(());
            }

            writer.write(&self.buffer[..size]);
        }
    }

    pub fn is_completed(&self) -> bool {
        self.bytes_left == 0
    }
}

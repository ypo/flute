use std::io::Write;

use super::{blockdecoder::BlockDecoder, lct, objectwriter::ObjectWriterSession};

pub struct BlockWriter {
    snb: u32,
    bytes_left: usize,
    cenc: lct::CENC,
}

impl BlockWriter {
    pub fn new(transfer_length: usize, cenc: lct::CENC) -> BlockWriter {
        BlockWriter {
            snb: 0,
            bytes_left: transfer_length,
            cenc,
        }
    }

    pub fn write(
        &mut self,
        snb: u32,
        block: &BlockDecoder,
        writer: &dyn ObjectWriterSession,
    ) -> bool {
        if self.snb != snb {
            return false;
        }
        assert!(block.completed);
        let data = block.source_encoding_symbols();
        for encoding_symbol in data {
            let symbols = encoding_symbol.as_ref().unwrap();
            let symbols = match self.bytes_left > symbols.len() {
                true => symbols.as_ref(),
                false => &symbols[..self.bytes_left],
            };
            writer.write(symbols);
            assert!(symbols.len() <= self.bytes_left);
            self.bytes_left -= symbols.len();
        }
        true
    }

    pub fn completed(&self) -> bool {
        self.bytes_left == 0
    }
}

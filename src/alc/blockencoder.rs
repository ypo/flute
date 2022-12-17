use std::rc::Rc;

use super::filedesc;
use super::pkt;
use crate::tools::error::Result;

pub struct BlockEncoder {
    file: Rc<filedesc::FileDesc>,
    curr_content_offset: u64,
    curr_snb: u32,
    a_large: u64,
    a_small: u64,
    nb_a_large: u64,
    nb_blocks: u64,
    blocks: Vec<Box<Block>>,
    block_interlace_windows: usize,
    block_interlace_index: usize,
    read_end: bool,
    source_size_transferred: usize,
}

use super::block::Block;

impl BlockEncoder {
    pub fn new(file: Rc<filedesc::FileDesc>, block_interlace_windows: usize) -> BlockEncoder {
        let mut block = BlockEncoder {
            file,
            curr_content_offset: 0,
            curr_snb: 0,
            a_large: 0,
            a_small: 0,
            nb_a_large: 0,
            nb_blocks: 0,
            blocks: Vec::new(),
            block_interlace_windows,
            block_interlace_index: 0,
            read_end: false,
            source_size_transferred: 0,
        };
        block.block_partitioning();
        block
    }

    pub fn read(&mut self) -> Option<pkt::Pkt> {
        loop {
            self.read_window();

            if self.blocks.is_empty() {
                return None;
            }

            if self.block_interlace_index >= self.blocks.len() {
                self.block_interlace_index = 0;
            }

            let block = &mut self.blocks[self.block_interlace_index];
            let symbol = block.read();
            if symbol.is_none() {
                log::debug!("End of the block");
                self.blocks.remove(self.block_interlace_index);
                continue;
            }

            let symbol = symbol.as_ref().unwrap();

            self.block_interlace_index += 1;
            if symbol.is_source_symbol {
                self.source_size_transferred += symbol.symbols.len();
            }

            return Some(pkt::Pkt {
                payload: symbol.symbols.to_vec(),
                transfer_length: self.file.object.transfer_length,
                esi: symbol.esi,
                snb: symbol.snb,
                toi: self.file.toi,
                fdt_id: self.file.fdt_id,
                cenc: self.file.object.cenc,
                inband_cenc: self.file.object.inband_cenc,
                close_object: self.source_size_transferred
                    >= self.file.object.transfer_length as usize,
            });
        }
    }

    fn block_partitioning(&mut self) {
        // https://tools.ietf.org/html/rfc5052
        // Block Partitioning Algorithm
        let oti = &self.file.oti;
        let b = oti.maximum_source_block_length as u64;
        let l = self.file.object.transfer_length;
        let e = oti.encoding_symbol_length as u64;

        let t = num_integer::div_ceil(l, e);
        let mut n = num_integer::div_ceil(t, b);
        if n == 0 {
            n = 1
        }

        self.a_large = num_integer::div_ceil(t, n);
        self.a_small = num_integer::div_floor(t, n);
        self.nb_a_large = t - (self.a_small * n);
        self.nb_blocks = n;
    }

    fn read_block(&mut self) -> Result<()> {
        assert!(self.read_end == false);
        if self.file.object.content.is_none() {
            self.read_end = true;
            return Ok(());
        }

        log::debug!("Read block nb {}", self.curr_snb);
        let oti = &self.file.oti;
        let content = self.file.object.content.as_ref().unwrap();
        let block_length = match self.curr_snb as u64 {
            value if value < self.nb_a_large => self.a_large,
            _ => self.a_small,
        };

        let offset_start = self.curr_content_offset as usize;
        let mut offset_end =
            offset_start + (block_length * oti.encoding_symbol_length as u64) as usize;
        if offset_end > content.len() {
            offset_end = content.len();
        }

        let buffer = &content.as_slice()[offset_start..offset_end];
        let block = Block::new_from_buffer(self.curr_snb, buffer, block_length, &oti)?;
        self.blocks.push(block);
        self.curr_snb += 1;
        self.read_end = offset_end == content.len();
        self.curr_content_offset = offset_end as u64;
        log::debug!(
            "offset={}/{} end={}",
            self.curr_content_offset,
            content.len(),
            self.read_end
        );

        Ok(())
    }

    fn read_window(&mut self) {
        while !self.read_end && (self.blocks.len() < self.block_interlace_windows) {
            match self.read_block() {
                Ok(_) => {}
                Err(_) => self.read_end = true, // TODO handle error
            };
        }
    }
}

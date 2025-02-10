use std::sync::Arc;

use super::filedesc;
use super::objectdesc::ObjectDataSource;
use crate::common::{partition, pkt};
use crate::error::FluteError;
use crate::tools::error::Result;

#[derive(Debug)]
pub struct BlockEncoder {
    file: Arc<filedesc::FileDesc>,
    curr_content_offset: u64,
    curr_sbn: u32,
    a_large: u64,
    a_small: u64,
    nb_a_large: u64,
    nb_blocks: u64,
    blocks: Vec<Box<Block>>,
    block_multiplex_windows: usize,
    block_multiplex_index: usize,
    read_end: bool,
    source_size_transferred: usize,
    nb_pkt_sent: usize,
    stopped: bool,
    closabled_object: bool,
}

use super::block::Block;

impl BlockEncoder {
    pub fn new(
        file: Arc<filedesc::FileDesc>,
        block_multiplex_windows: usize,
        closabled_object: bool,
    ) -> Result<BlockEncoder> {
        match &file.object.source {
            ObjectDataSource::Buffer(_) => {}
            ObjectDataSource::Stream(stream) => {
                stream.lock().unwrap().seek(std::io::SeekFrom::Start(0))?;
            }
        }

        let mut block = BlockEncoder {
            file,
            curr_content_offset: 0,
            curr_sbn: 0,
            a_large: 0,
            a_small: 0,
            nb_a_large: 0,
            nb_blocks: 0,
            blocks: Vec::new(),
            block_multiplex_windows,
            block_multiplex_index: 0,
            read_end: false,
            source_size_transferred: 0,
            nb_pkt_sent: 0,
            stopped: false,
            closabled_object,
        };
        block.block_partitioning();
        Ok(block)
    }

    pub fn read(&mut self, force_close_object: bool) -> Option<pkt::Pkt> {
        if self.stopped {
            return None;
        }

        if force_close_object {
            self.stopped = true;
        }

        loop {
            self.read_window();

            if self.blocks.is_empty() {
                if self.nb_pkt_sent == 0 {
                    log::debug!("Empty file ? Send a pkt containing close object flag");
                    self.nb_pkt_sent += 1;

                    debug_assert!(self.file.object.transfer_length == 0);
                    return Some(pkt::Pkt {
                        payload: Vec::new(),
                        transfer_length: self.file.object.transfer_length,
                        esi: 0,
                        sbn: 0,
                        toi: self.file.toi,
                        fdt_id: self.file.fdt_id,
                        cenc: self.file.object.cenc,
                        inband_cenc: self.file.object.inband_cenc,
                        close_object: true,
                        source_block_length: 0,
                        sender_current_time: self.file.sender_current_time,
                    });
                }

                return None;
            }

            if self.block_multiplex_index >= self.blocks.len() {
                self.block_multiplex_index = 0;
            }

            let block = &mut self.blocks[self.block_multiplex_index];
            let symbol = block.read();
            if symbol.is_none() {
                self.blocks.remove(self.block_multiplex_index);
                continue;
            }

            let (symbol, is_last_symbol) = symbol.as_ref().unwrap();

            self.block_multiplex_index += 1;
            if symbol.is_source_symbol {
                self.source_size_transferred += symbol.symbols.len();
            }

            self.nb_pkt_sent += 1;

            let is_last_packet = (self.source_size_transferred
                >= self.file.object.transfer_length as usize)
                && *is_last_symbol;

            return Some(pkt::Pkt {
                payload: symbol.symbols.to_vec(),
                transfer_length: self.file.object.transfer_length,
                esi: symbol.esi,
                sbn: symbol.sbn,
                toi: self.file.toi,
                fdt_id: self.file.fdt_id,
                cenc: self.file.object.cenc,
                inband_cenc: self.file.object.inband_cenc,
                close_object: force_close_object || (self.closabled_object && is_last_packet),
                source_block_length: block.nb_source_symbols as u32,
                sender_current_time: self.file.sender_current_time,
            });
        }
    }

    fn block_partitioning(&mut self) {
        let oti = &self.file.oti;
        (self.a_large, self.a_small, self.nb_a_large, self.nb_blocks) =
            partition::block_partitioning(
                oti.maximum_source_block_length as u64,
                self.file.object.transfer_length,
                oti.encoding_symbol_length as u64,
            );
    }

    fn read_block(&mut self) -> Result<()> {
        debug_assert!(!self.read_end);
        let source = &self.file.object.source;
        match source {
            ObjectDataSource::Buffer(_) => self.read_block_buffer(),
            ObjectDataSource::Stream(_) => self.read_block_stream(),
        }
    }

    fn read_block_buffer(&mut self) -> Result<()> {
        log::debug!("Read block nb {}", self.curr_sbn);

        let content = match &self.file.object.source {
            ObjectDataSource::Buffer(buffer) => Ok(buffer),
            _ => Err(FluteError::new("Not a data source buffer")),
        }?;

        let oti = &self.file.oti;
        let block_length = match self.curr_sbn as u64 {
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
        let block = Block::new_from_buffer(self.curr_sbn, buffer, block_length, oti)?;
        self.blocks.push(block);
        self.curr_sbn += 1;
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

    fn read_block_stream(&mut self) -> Result<()> {
        log::info!("Read block nb {}", self.curr_sbn);

        let mut stream = match &self.file.object.source {
            ObjectDataSource::Stream(stream) => Ok(stream.lock().unwrap()),
            _ => Err(FluteError::new("Not a data source stream")),
        }?;

        let oti = &self.file.oti;
        let block_length = match self.curr_sbn as u64 {
            value if value < self.nb_a_large => self.a_large,
            _ => self.a_small,
        };
        let mut buffer: Vec<u8> =
            vec![0; block_length as usize * oti.encoding_symbol_length as usize];
        let result = match stream.read(&mut buffer) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Fail to read file {:?}", e.to_string());
                self.read_end = true;
                return Ok(());
            }
        };

        if result == 0 {
            self.read_end = true;
            return Ok(());
        }

        buffer.truncate(result);

        let block = Block::new_from_buffer(self.curr_sbn, &buffer, block_length, oti)?;
        self.blocks.push(block);
        self.curr_sbn += 1;
        self.curr_content_offset += buffer.len() as u64;
        Ok(())
    }

    fn read_window(&mut self) {
        while !self.read_end && (self.blocks.len() < self.block_multiplex_windows) {
            match self.read_block() {
                Ok(_) => {}
                Err(_) => self.read_end = true, // TODO handle error
            };
        }
    }
}

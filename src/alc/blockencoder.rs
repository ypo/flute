use std::rc::Rc;

use super::filedesc;
use super::pkt;

pub struct BlockEncoder {
    file: Rc<filedesc::FileDesc>,
}

impl BlockEncoder {
    pub fn new(file: Rc<filedesc::FileDesc>) -> BlockEncoder {
        BlockEncoder { file }
    }

    pub fn read() -> Option<pkt::Pkt> {
        None
    }

    fn block_partitioning(&mut self) {
        // https://tools.ietf.org/html/rfc5052
        // Block Partitioning Algorithm
        let oti = &self.file.oti;
        let b = oti.maximum_source_block_length;
        let l = self.file.object.transfer_length;
        let e = oti.encoding_symbol_length;
    }
}

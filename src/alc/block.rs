struct Block {
    block_length: u32,
    snb: u32,
    esi: u32,
    data: Vec<u8>,
}

impl Block {
    pub fn new() -> Block {
        Block {
            block_length: 0,
            snb: 0,
            esi: 0,
            data: Vec::new(),
        }
    }

    pub fn open(&mut self, block_length: u32, snb: u32) {
        self.block_length = block_length;
        self.snb = snb;
        self.esi = 0
    }

    pub fn close(&mut self) {
        self.data.clear();
    }

    pub fn fill(&mut self) {}
}

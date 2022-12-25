#[derive(Debug)]
pub struct RingBuffer {
    buffer: Vec<u8>,
    producer: usize,
    consumer: usize,
    finish: bool,
}

impl RingBuffer {
    pub fn new(size: usize) -> Self {
        let mut buffer = Vec::with_capacity(size);
        buffer.resize(size, 0);
        Self {
            buffer,
            producer: 0,
            consumer: 0,
            finish: false,
        }
    }

    pub fn finish(&mut self) {
        self.finish = true;
    }

    fn write_size(&self) -> usize {
        if self.producer < self.consumer {
            return self.consumer - self.producer - 1;
        }

        self.buffer.len() - self.producer + self.consumer - 1
    }

    fn read_size(&self) -> usize {
        if self.consumer <= self.producer {
            return self.producer - self.consumer;
        }

        self.buffer.len() - self.consumer + self.producer
    }
}

impl std::io::Read for RingBuffer {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut max_size = self.read_size();
        if max_size > buf.len() {
            max_size = buf.len();
        }

        if max_size == 0 {
            if self.finish == true {
                return Ok(0);
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "waiting for more data",
            ));
        }

        if self.consumer < self.producer {
            buf[..max_size]
                .copy_from_slice(&self.buffer[self.consumer..(self.consumer + max_size)]);
            self.consumer += max_size;
            return Ok(max_size);
        }

        let end_size = self.buffer.len() - self.consumer;
        if end_size > max_size {
            buf[..max_size]
                .copy_from_slice(&self.buffer[self.consumer..(self.consumer + max_size)]);
            self.consumer += max_size;
            assert!(self.consumer <= self.buffer.len());

            if self.consumer == self.buffer.len() {
                self.consumer = 0;
            }

            return Ok(max_size);
        }

        buf[..end_size].copy_from_slice(&self.buffer[self.consumer..(self.consumer + end_size)]);
        let left = max_size - end_size;
        self.consumer = 0;

        buf[end_size..(end_size + left)].copy_from_slice(&self.buffer[..left]);
        self.consumer += left;
        assert!(self.consumer <= self.producer);
        assert!(self.consumer != self.buffer.len());
        Ok(max_size)
    }
}

impl std::io::Write for RingBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut max_size = self.write_size();
        if max_size == 0 {
            return Ok(0);
        }

        if max_size > buf.len() {
            max_size = buf.len();
        }

        if self.consumer > self.producer {
            self.buffer[self.producer..(self.producer + max_size)]
                .copy_from_slice(&buf[..max_size]);
            self.producer += max_size;
            assert!(self.consumer < self.producer);
            assert!(self.producer != self.buffer.len());
            return Ok(max_size);
        }

        let end_size = self.buffer.len() - self.producer;
        if end_size >= max_size {
            self.buffer[self.producer..(self.producer + max_size)]
                .copy_from_slice(&buf[..max_size]);
            self.producer += max_size;

            if self.producer == self.buffer.len() {
                self.producer = 0;
            }

            return Ok(max_size);
        }

        self.buffer[self.producer..(self.producer + end_size)].copy_from_slice(&buf[..end_size]);
        self.producer = 0;
        let left_size = max_size - end_size;
        self.buffer[..left_size].copy_from_slice(&buf[end_size..(end_size + left_size)]);
        self.producer += left_size;
        assert!(self.producer < self.consumer);
        Ok(max_size)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

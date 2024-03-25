#[derive(Debug)]
pub struct RingBuffer {
    buffer: Vec<u8>,
    producer: usize,
    consumer: usize,
    finish: bool,
}

impl RingBuffer {
    pub fn new(size: usize) -> Self {
        let buffer = vec![0; size];
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
            if self.finish {
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
        if end_size >= max_size {
            buf[..max_size]
                .copy_from_slice(&self.buffer[self.consumer..(self.consumer + max_size)]);
            self.consumer += max_size;
            debug_assert!(self.consumer <= self.buffer.len());

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
        debug_assert!(self.consumer <= self.producer);
        debug_assert!(self.consumer != self.buffer.len());
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
            debug_assert!(self.consumer > self.producer);
            debug_assert!(self.producer != self.buffer.len());
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
        debug_assert!(self.producer < self.consumer);
        Ok(max_size)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use std::io::Read;
    use std::io::Write;

    #[test]
    pub fn ringbuffer() {
        crate::tests::init();
        const RING_SIZE: usize = 1024;
        let mut ring = super::RingBuffer::new(RING_SIZE);

        let mut buffer: Vec<u8> = vec![0; RING_SIZE / 3];

        assert!(ring.write_size() < RING_SIZE);

        let wsize = ring.write(buffer.as_ref()).unwrap();
        assert!(wsize == buffer.len());

        let rsize = ring.read_size();
        assert!(rsize == buffer.len());

        let rsize = ring.read(buffer.as_mut()).unwrap();
        assert!(rsize == buffer.len());
        assert!(ring.read_size() == 0);

        ring.write(&buffer[0..buffer.len()]).unwrap();
        ring.write(&buffer[0..buffer.len() / 2]).unwrap();

        let rsize = ring.read(buffer.as_mut()).unwrap();
        assert!(rsize == buffer.len());

        loop {
            let wsize = ring.write(buffer.as_ref()).unwrap();
            if wsize == 0 {
                break;
            }
        }
    }

    #[test]
    pub fn ringbuffer_2() {
        crate::tests::init();
        const RING_SIZE: usize = 3;
        let mut buffer: Vec<u8> = vec![0; 1024];
        let mut ring = super::RingBuffer::new(RING_SIZE);

        for _ in 0..10 {
            let wsize = ring.write(buffer.as_ref()).unwrap();
            assert!(wsize == RING_SIZE - 1);

            let rsize = ring.read_size();
            assert!(rsize == RING_SIZE - 1);

            let rsize = ring.read(buffer.as_mut()).unwrap();
            assert!(rsize == RING_SIZE - 1);

            let wouldblock = ring.read(buffer.as_mut());
            assert!(wouldblock.is_err());
            assert!(wouldblock.err().unwrap().kind() == std::io::ErrorKind::WouldBlock);
        }
    }

    #[test]
    pub fn ringbuffer_3() {
        crate::tests::init();
        const RING_SIZE: usize = 11;
        let mut wbuffer: Vec<u8> = vec![0xAA; 9];
        let mut rbuffer: Vec<u8> = vec![0; 1];

        let mut ring = super::RingBuffer::new(RING_SIZE);

        ring.write(wbuffer.as_ref()).unwrap();
        ring.read(wbuffer.as_mut()).unwrap();
        assert!(ring.read_size() == 0);

        ring.write(wbuffer.as_ref()).unwrap();

        for _ in 0..25 {
            ring.write(wbuffer.as_ref()).unwrap();
            ring.flush().unwrap();
            loop {
                match ring.read(rbuffer.as_mut()) {
                    Ok(res) => {
                        assert!(res == 1);
                        assert!(rbuffer[0] == 0xAA);
                    }
                    Err(e) => {
                        assert!(e.kind() == std::io::ErrorKind::WouldBlock);
                        break;
                    }
                }
            }
            rbuffer.fill(0);
        }
    }
}

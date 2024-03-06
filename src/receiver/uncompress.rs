use crate::tools::ringbuffer::RingBuffer;
use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use std::io::Read;
use std::io::Write;

pub trait Decompress {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize>;
    fn read(&mut self, data: &mut [u8]) -> std::io::Result<usize>;
    fn finish(&mut self);
}

impl std::fmt::Debug for dyn Decompress {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Decompress {{  }}")
    }
}

pub struct DecompressGzip {
    decoder: GzDecoder<RingBuffer>,
}

impl DecompressGzip {
    pub fn new(pkt: &[u8]) -> DecompressGzip {
        let mut ring = RingBuffer::new(pkt.len() * 2);
        let result = ring.write(pkt).unwrap();
        debug_assert!(result == pkt.len());
        DecompressGzip {
            decoder: GzDecoder::new(ring),
        }
    }
}

impl Decompress for DecompressGzip {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.decoder.get_mut().write(data)
    }

    fn read(&mut self, data: &mut [u8]) -> std::io::Result<usize> {
        self.decoder.read(data)
    }
    fn finish(&mut self) {
        self.decoder.get_mut().finish();
    }
}

pub struct DecompressDeflate {
    decoder: DeflateDecoder<RingBuffer>,
}

impl DecompressDeflate {
    pub fn new(pkt: &[u8]) -> DecompressDeflate {
        let mut ring = RingBuffer::new(pkt.len() * 2);
        let result = ring.write(pkt).unwrap();
        debug_assert!(result == pkt.len());
        DecompressDeflate {
            decoder: DeflateDecoder::new(ring),
        }
    }
}

impl Decompress for DecompressDeflate {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.decoder.get_mut().write(data)
    }

    fn read(&mut self, data: &mut [u8]) -> std::io::Result<usize> {
        self.decoder.read(data)
    }
    fn finish(&mut self) {
        self.decoder.get_mut().finish();
    }
}

pub struct DecompressZlib {
    decoder: ZlibDecoder<RingBuffer>,
}

impl DecompressZlib {
    pub fn new(pkt: &[u8]) -> DecompressZlib {
        let mut ring = RingBuffer::new(pkt.len() * 2);
        let result = ring.write(pkt).unwrap();
        debug_assert!(result == pkt.len());
        DecompressZlib {
            decoder: ZlibDecoder::new(ring),
        }
    }
}

impl Decompress for DecompressZlib {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.decoder.get_mut().write(data)
    }

    fn read(&mut self, data: &mut [u8]) -> std::io::Result<usize> {
        self.decoder.read(data)
    }
    fn finish(&mut self) {
        self.decoder.get_mut().finish();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    pub fn test_gzip() {}
}

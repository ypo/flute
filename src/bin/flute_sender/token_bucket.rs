use std::time::Instant;

/// A token bucket rate limiter.
///
/// Tokens are added at a fixed rate (bytes per second). Each packet sent
/// consumes tokens equal to its size. When the bucket is empty the caller
/// sleeps until enough tokens are available.
pub struct TokenBucket {
    rate: f64,         // tokens (bytes) per second
    capacity: f64,     // max burst size in bytes
    tokens: f64,       // current available tokens
    last: Instant,     // last refill timestamp
    packet_size: f64,  // expected packet size in bytes
}

impl TokenBucket {
    /// Create a new token bucket.
    ///
    /// * `bits_per_second` – target transmission rate
    /// * `burst_bytes`     – max burst size in bytes (bucket capacity)
    /// * `packet_size`     – expected packet size in bytes (wait threshold)
    pub fn new(bits_per_second: u64, burst_bytes: u64, packet_size: u16) -> Self {
        let rate = bits_per_second as f64 / 8.0;
        let capacity = burst_bytes as f64;
        Self {
            rate,
            capacity,
            tokens: packet_size as f64, // start with one packet to avoid initial burst
            last: Instant::now(),
            packet_size: packet_size as f64,
        }
    }

    /// Block until at least `packet_size` bytes worth of tokens is available.
    pub fn wait_for_capacity(&mut self) {
        self.refill();
        if self.tokens < self.packet_size {
            let deficit = self.packet_size - self.tokens;
            let wait = std::time::Duration::from_secs_f64(deficit / self.rate);
            std::thread::sleep(wait);
            self.refill();
        }
    }

    /// Deduct `size` bytes from the bucket. Tokens may go negative;
    /// the next `wait_for_capacity` call will block to compensate.
    pub fn consume(&mut self, size: usize) {
        self.tokens -= size as f64;
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.rate).min(self.capacity);
        self.last = now;
    }
}

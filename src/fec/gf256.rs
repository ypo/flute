//! Multiply-accumulate of byte slices over GF(2^8), dst += c * src,
//! with the primitive polynomial 1 + x^2 + x^3 + x^4 + x^8 (RFC 5510 §8.1).
//!
//! The product c * x is split into c * (x & 0x0F) + c * (x & 0xF0): two lookups in tables of 16 entries,
//! that byte shuffle instructions evaluate 16 or 32 bytes at a time.
//! AVX2 or SSSE3 are selected at runtime on x86, NEON is used on aarch64.

use std::sync::OnceLock;

/// Implementation of [`mul_add`]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kernel {
    /// Lookup in a multiplication table, one byte at a time
    Scalar,
    /// x86 SSSE3, 16 bytes at a time
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    Ssse3,
    /// x86 AVX2, 32 bytes at a time
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    Avx2,
    /// aarch64 NEON, 16 bytes at a time
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    Neon,
}

impl Kernel {
    /// Kernels supported by the CPU, the fastest first
    pub fn available() -> Vec<Kernel> {
        let mut kernels = Vec::new();
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            if is_x86_feature_detected!("avx2") {
                kernels.push(Kernel::Avx2);
            }
            if is_x86_feature_detected!("ssse3") {
                kernels.push(Kernel::Ssse3);
            }
        }
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        kernels.push(Kernel::Neon);
        kernels.push(Kernel::Scalar);
        kernels
    }

    /// Fastest kernel supported by the CPU
    pub fn best() -> Kernel {
        static BEST: OnceLock<Kernel> = OnceLock::new();
        *BEST.get_or_init(|| Kernel::available()[0])
    }
}

/// a * b, by shift and add
fn mul_slow(mut a: u8, mut b: u8) -> u8 {
    let mut product = 0;
    while b != 0 {
        if b & 1 != 0 {
            product ^= a;
        }
        let carry = a & 0x80 != 0;
        a <<= 1;
        if carry {
            a ^= 0x1D;
        }
        b >>= 1;
    }
    product
}

/// table\[a\]\[b\] = a * b
fn mul_table() -> &'static [[u8; 256]] {
    static TABLE: OnceLock<Vec<[u8; 256]>> = OnceLock::new();
    TABLE.get_or_init(|| {
        (0..=255u8)
            .map(|a| std::array::from_fn(|b| mul_slow(a, b as u8)))
            .collect()
    })
}

/// a * b
#[cfg(test)]
pub fn mul(a: u8, b: u8) -> u8 {
    mul_table()[a as usize][b as usize]
}

/// c * x = low\[x & 0x0F\] + high\[x >> 4\]
#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    all(target_arch = "aarch64", target_feature = "neon")
))]
struct NibbleTables {
    low: [u8; 16],
    high: [u8; 16],
}

#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    all(target_arch = "aarch64", target_feature = "neon")
))]
impl NibbleTables {
    fn new(row: &[u8; 256]) -> Self {
        NibbleTables {
            low: std::array::from_fn(|i| row[i]),
            high: std::array::from_fn(|i| row[i << 4]),
        }
    }
}

/// dst\[i\] += c * src\[i\], for i < min(dst.len(), src.len())
pub fn mul_add(dst: &mut [u8], src: &[u8], c: u8) {
    mul_add_with(Kernel::best(), dst, src, c);
}

/// [`mul_add`] with a given kernel, or with [`Kernel::Scalar`] if the CPU does not support it
pub fn mul_add_with(kernel: Kernel, dst: &mut [u8], src: &[u8], c: u8) {
    let len = dst.len().min(src.len());
    let (dst, src) = (&mut dst[..len], &src[..len]);
    match c {
        0 => return,
        1 => {
            // Auto-vectorized
            dst.iter_mut().zip(src).for_each(|(d, s)| *d ^= s);
            return;
        }
        _ => {}
    }

    let row = &mul_table()[c as usize];
    let done = match kernel {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Kernel::Avx2 if is_x86_feature_detected!("avx2") => {
            // SAFETY: the CPU supports AVX2
            unsafe { x86::mul_add_avx2(dst, src, &NibbleTables::new(row)) }
        }
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Kernel::Ssse3 if is_x86_feature_detected!("ssse3") => {
            // SAFETY: the CPU supports SSSE3
            unsafe { x86::mul_add_ssse3(dst, src, &NibbleTables::new(row)) }
        }
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        Kernel::Neon => neon::mul_add(dst, src, &NibbleTables::new(row)),
        _ => 0,
    };

    for (d, &s) in dst[done..].iter_mut().zip(&src[done..]) {
        *d ^= row[s as usize];
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod x86 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    use super::NibbleTables;

    /// Processes the first multiple of 32 bytes of `dst` and `src` (of the same length),
    /// returns the number of bytes processed
    ///
    /// # Safety
    ///
    /// The CPU must support AVX2
    #[target_feature(enable = "avx2")]
    pub unsafe fn mul_add_avx2(dst: &mut [u8], src: &[u8], tables: &NibbleTables) -> usize {
        debug_assert_eq!(dst.len(), src.len());
        // SAFETY: the loads and stores are within 16-byte arrays and 32-byte chunks
        unsafe {
            let low = _mm256_broadcastsi128_si256(_mm_loadu_si128(tables.low.as_ptr().cast()));
            let high = _mm256_broadcastsi128_si256(_mm_loadu_si128(tables.high.as_ptr().cast()));
            let mask = _mm256_set1_epi8(0x0F);
            for (d, s) in dst.chunks_exact_mut(32).zip(src.chunks_exact(32)) {
                let x = _mm256_loadu_si256(s.as_ptr().cast());
                let product = _mm256_xor_si256(
                    _mm256_shuffle_epi8(low, _mm256_and_si256(x, mask)),
                    _mm256_shuffle_epi8(high, _mm256_and_si256(_mm256_srli_epi64::<4>(x), mask)),
                );
                let y = _mm256_loadu_si256(d.as_ptr().cast());
                _mm256_storeu_si256(d.as_mut_ptr().cast(), _mm256_xor_si256(y, product));
            }
        }
        dst.len() / 32 * 32
    }

    /// Processes the first multiple of 16 bytes of `dst` and `src` (of the same length),
    /// returns the number of bytes processed
    ///
    /// # Safety
    ///
    /// The CPU must support SSSE3
    #[target_feature(enable = "ssse3")]
    pub unsafe fn mul_add_ssse3(dst: &mut [u8], src: &[u8], tables: &NibbleTables) -> usize {
        debug_assert_eq!(dst.len(), src.len());
        // SAFETY: the loads and stores are within 16-byte arrays and 16-byte chunks
        unsafe {
            let low = _mm_loadu_si128(tables.low.as_ptr().cast());
            let high = _mm_loadu_si128(tables.high.as_ptr().cast());
            let mask = _mm_set1_epi8(0x0F);
            for (d, s) in dst.chunks_exact_mut(16).zip(src.chunks_exact(16)) {
                let x = _mm_loadu_si128(s.as_ptr().cast());
                let product = _mm_xor_si128(
                    _mm_shuffle_epi8(low, _mm_and_si128(x, mask)),
                    _mm_shuffle_epi8(high, _mm_and_si128(_mm_srli_epi64::<4>(x), mask)),
                );
                let y = _mm_loadu_si128(d.as_ptr().cast());
                _mm_storeu_si128(d.as_mut_ptr().cast(), _mm_xor_si128(y, product));
            }
        }
        dst.len() / 16 * 16
    }
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
mod neon {
    use std::arch::aarch64::*;

    use super::NibbleTables;

    /// Processes the first multiple of 16 bytes of `dst` and `src` (of the same length),
    /// returns the number of bytes processed
    pub fn mul_add(dst: &mut [u8], src: &[u8], tables: &NibbleTables) -> usize {
        debug_assert_eq!(dst.len(), src.len());
        // SAFETY: NEON is enabled at compile time,
        // the loads and stores are within 16-byte arrays and 16-byte chunks
        unsafe {
            let low = vld1q_u8(tables.low.as_ptr());
            let high = vld1q_u8(tables.high.as_ptr());
            let mask = vdupq_n_u8(0x0F);
            for (d, s) in dst.chunks_exact_mut(16).zip(src.chunks_exact(16)) {
                let x = vld1q_u8(s.as_ptr());
                let product = veorq_u8(
                    vqtbl1q_u8(low, vandq_u8(x, mask)),
                    vqtbl1q_u8(high, vshrq_n_u8::<4>(x)),
                );
                vst1q_u8(d.as_mut_ptr(), veorq_u8(vld1q_u8(d.as_ptr()), product));
            }
        }
        dst.len() / 16 * 16
    }
}

#[cfg(test)]
mod tests {
    use super::{mul, mul_add_with, Kernel};

    #[test]
    pub fn test_mul() {
        crate::tests::init();
        for a in 0..=255u8 {
            assert_eq!(mul(a, 0), 0);
            assert_eq!(mul(a, 1), a);
            for b in 0..=255u8 {
                assert_eq!(mul(a, b), mul(b, a));
            }
        }
        // alpha^8 = 1 + alpha^2 + alpha^3 + alpha^4
        assert_eq!(mul(0x80, 2), 0x1D);
    }

    #[test]
    pub fn test_kernels() {
        crate::tests::init();
        let kernels = Kernel::available();
        log::info!("GF(2^8) kernels: {:?}", kernels);
        assert_eq!(kernels.last(), Some(&Kernel::Scalar));
        assert_eq!(Kernel::best(), kernels[0]);

        let src: Vec<u8> = (0..1100usize).map(|i| (i * 167 + i / 3) as u8).collect();
        let init: Vec<u8> = (0..1100usize).map(|i| (i * 13 + 7) as u8).collect();
        for kernel in kernels {
            for c in [0u8, 1, 2, 0x1D, 0x53, 0x80, 0xFF] {
                for (offset, len) in [(0, 0), (0, 1), (1, 15), (3, 16), (0, 31), (5, 32), (7, 33), (1, 100), (0, 1024), (2, 1097)] {
                    let mut dst = init.clone();
                    mul_add_with(kernel, &mut dst[offset..], &src[offset + 1..offset + 1 + len], c);
                    let expected: Vec<u8> = init
                        .iter()
                        .enumerate()
                        .map(|(i, &d)| match i >= offset && i < offset + len {
                            true => d ^ mul(c, src[i + 1]),
                            false => d,
                        })
                        .collect();
                    assert_eq!(dst, expected, "{:?} c={} offset={} len={}", kernel, c, offset, len);
                }
            }
        }
    }
}

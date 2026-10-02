//! Reed-Solomon codes over GF(2^m), m in {2..16}
//! <https://www.rfc-editor.org/rfc/rfc5510.html#section-8>
//!
//! Used by FEC Encoding ID 2, and with m = 8 by FEC Encoding ID 5 and FEC Encoding ID 129 / FEC Instance ID 0.
//!
//! The generator matrix is GM = V_{k,k}^-1 * V_{k,n} (RFC 5510 §8.2.1), built from the Vandermonde matrix
//! of the codec of Luigi Rizzo, which RFC 5510 declares to be compatible with:
//! v_{i,j} = x_j^i, with the evaluation points x_0 = 0 and x_j = alpha^(j-1) for j > 0.
//! The literal text of §8.2.1 (v_{i,j} = alpha^(i*j), i.e. x_j = alpha^j) produces other repair symbols,
//! that the reference implementations (Rizzo's codec, zfec, OpenFEC) do not produce.
//! The compatibility with zfec and OpenFEC can be checked with `interop/rs/run.sh`.
//!
//! The encoding element j is therefore the value at x_j of the polynomial of degree lower than k
//! that takes the k source elements at x_0..x_{k-1}.
//! Repair symbols (encoding) and missing source symbols (decoding) are computed by Lagrange interpolation.
//!
//! An encoding symbol of E bytes is made of S = 8 * E / m elements of m bits, most significant bit first (RFC 5510 §8.4).
//! With m = 8, the symbols are combined with the SIMD kernels of [`super::gf256`].

use std::collections::BTreeMap;
use std::sync::OnceLock;

use super::{gf256, DataFecShard, FecDecoder, FecEncoder, FecShard};
use crate::common::oti::ReedSolomonGF2MSchemeSpecific;
use crate::tools::error::{FluteError, Result};

/// Primitive polynomials of RFC 5510 §8.1, indexed by m
const PRIMITIVE_POLYNOMIALS: [u32; 17] = [
    0, 0, 0x7, 0xB, 0x13, 0x25, 0x43, 0x89, 0x11D, 0x211, 0x409, 0x805, 0x1053, 0x201B, 0x4443,
    0x8003, 0x1100B,
];

/// Check the parameters of a Reed-Solomon code over GF(2^m)
///
/// * m is between 2 and 16
/// * 0 < k <= n <= 2^m - 1 <https://www.rfc-editor.org/rfc/rfc5510.html#section-6.1>
/// * an encoding symbol is made of an integer number of m-bit elements
pub fn check_parameters(
    m: u8,
    nb_source_symbols: usize,
    nb_encoding_symbols: usize,
    encoding_symbol_length: usize,
) -> Result<()> {
    if !(2..=16).contains(&m) {
        return Err(FluteError::new(format!(
            "Reed-Solomon GF(2^m): m={} is not between 2 and 16",
            m
        )));
    }

    if nb_source_symbols == 0 {
        return Err(FluteError::new(
            "Reed-Solomon GF(2^m): source block length must be at least 1",
        ));
    }

    let max_nb_encoding_symbols = (1usize << m) - 1;
    if nb_encoding_symbols < nb_source_symbols || nb_encoding_symbols > max_nb_encoding_symbols {
        return Err(FluteError::new(format!(
            "Reed-Solomon GF(2^m): number of encoding symbols {} must be between the source block length {} and 2^m - 1 = {}",
            nb_encoding_symbols, nb_source_symbols, max_nb_encoding_symbols
        )));
    }

    if encoding_symbol_length == 0 || (encoding_symbol_length * 8) % m as usize != 0 {
        return Err(FluteError::new(format!(
            "Reed-Solomon GF(2^m): encoding symbol length of {} bytes is not a multiple of m={} bits",
            encoding_symbol_length, m
        )));
    }

    Ok(())
}

struct GaloisField {
    m: u8,
    /// exp\[i\] = alpha^i, for i in 0..2 * (2^m - 1)
    exp: Vec<u16>,
    /// log\[alpha^i\] = i, log\[0\] is not used
    log: Vec<u16>,
}

impl std::fmt::Debug for GaloisField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GF(2^{})", self.m)
    }
}

impl GaloisField {
    fn get(m: u8) -> &'static GaloisField {
        static FIELDS: [OnceLock<GaloisField>; 17] = [const { OnceLock::new() }; 17];
        FIELDS[m as usize].get_or_init(|| GaloisField::new(m))
    }

    fn new(m: u8) -> GaloisField {
        debug_assert!((2..=16).contains(&m));
        let order = (1usize << m) - 1;
        let polynomial = PRIMITIVE_POLYNOMIALS[m as usize];
        let mut exp = vec![0u16; 2 * order];
        let mut log = vec![0u16; order + 1];
        let mut x: u32 = 1;
        for i in 0..order {
            exp[i] = x as u16;
            exp[i + order] = x as u16;
            log[x as usize] = i as u16;
            x <<= 1;
            if x >> m != 0 {
                x ^= polynomial;
            }
        }
        GaloisField { m, exp, log }
    }

    /// Number of non-zero elements (2^m - 1)
    fn order(&self) -> usize {
        self.exp.len() / 2
    }

    fn alpha_pow(&self, exponent: usize) -> u16 {
        self.exp[exponent % self.order()]
    }

    /// Evaluation point x_esi of the encoding symbol `esi` < 2^m: 0, alpha^0, alpha^1, ...
    fn point(&self, esi: usize) -> u16 {
        match esi {
            0 => 0,
            _ => self.alpha_pow(esi - 1),
        }
    }

    fn mul(&self, a: u16, b: u16) -> u16 {
        if a == 0 || b == 0 {
            return 0;
        }
        self.exp[self.log[a as usize] as usize + self.log[b as usize] as usize]
    }

    fn inv(&self, a: u16) -> u16 {
        debug_assert!(a != 0);
        self.exp[self.order() - self.log[a as usize] as usize]
    }

    /// dst += c * src
    fn add_mul(&self, dst: &mut [u16], src: &[u16], c: u16) {
        if c == 0 {
            return;
        }
        let log_c = self.log[c as usize] as usize;
        for (d, &s) in dst.iter_mut().zip(src) {
            if s != 0 {
                *d ^= self.exp[log_c + self.log[s as usize] as usize];
            }
        }
    }

    /// Lagrange interpolation of the polynomial of degree lower than `points.len()`
    ///
    /// Returns the matrix `coefs` such that the encoding element targets\[t\] is
    /// the sum of `coefs[t][p]` * the encoding element points\[p\].
    ///
    /// `points` and `targets` are disjoint sets of ESIs lower than 2^m
    fn interpolation_matrix(&self, points: &[usize], targets: &[usize]) -> Vec<Vec<u16>> {
        let x: Vec<u16> = points.iter().map(|&p| self.point(p)).collect();

        // 1 / prod_{l != p} (x_p - x_l)
        let inv_denominators: Vec<u16> = x
            .iter()
            .enumerate()
            .map(|(p, &xp)| {
                let denominator = x
                    .iter()
                    .enumerate()
                    .filter(|&(l, _)| l != p)
                    .fold(1, |acc, (_, &xl)| self.mul(acc, xp ^ xl));
                self.inv(denominator)
            })
            .collect();

        targets
            .iter()
            .map(|&t| {
                let xt = self.point(t);
                // prod_l (x_t - x_l)
                let numerator = x.iter().fold(1, |acc, &xl| self.mul(acc, xt ^ xl));
                x.iter()
                    .zip(&inv_denominators)
                    .map(|(&xp, &inv_denominator)| {
                        self.mul(self.mul(numerator, self.inv(xt ^ xp)), inv_denominator)
                    })
                    .collect()
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct RSGalois2MCodec {
    field: &'static GaloisField,
    nb_source_symbols: usize,
    nb_encoding_symbols: usize,
    encoding_symbol_length: usize,
    nb_symbols_per_group: usize,
    received: BTreeMap<usize, Vec<u8>>,
    decode_block: Option<Vec<u8>>,
}

impl RSGalois2MCodec {
    /// * `nb_source_symbols`: k
    /// * `nb_encoding_symbols`: n, symbols with an ESI greater or equal are ignored by the decoder
    pub fn new(
        nb_source_symbols: usize,
        nb_encoding_symbols: usize,
        encoding_symbol_length: usize,
        scheme: &ReedSolomonGF2MSchemeSpecific,
    ) -> Result<RSGalois2MCodec> {
        check_parameters(
            scheme.m,
            nb_source_symbols,
            nb_encoding_symbols,
            encoding_symbol_length,
        )?;

        Ok(RSGalois2MCodec {
            field: GaloisField::get(scheme.m),
            nb_source_symbols,
            nb_encoding_symbols,
            encoding_symbol_length,
            nb_symbols_per_group: scheme.g.max(1) as usize,
            received: BTreeMap::new(),
            decode_block: None,
        })
    }

    /// S
    fn nb_elements(&self) -> usize {
        self.encoding_symbol_length * 8 / self.field.m as usize
    }

    /// Split a symbol into S m-bit elements, a symbol shorter than E is padded with zeros
    fn to_elements(&self, symbol: &[u8]) -> Vec<u16> {
        let m = self.field.m as u32;
        let mask = (1u32 << m) - 1;
        let mut elements = Vec::with_capacity(self.nb_elements());
        let (mut acc, mut nb_bits) = (0u32, 0u32);
        for &byte in symbol {
            acc = (acc << 8) | byte as u32;
            nb_bits += 8;
            while nb_bits >= m {
                nb_bits -= m;
                elements.push(((acc >> nb_bits) & mask) as u16);
            }
            acc &= (1 << nb_bits) - 1;
        }
        if nb_bits > 0 {
            elements.push(((acc << (m - nb_bits)) & mask) as u16);
        }
        elements.resize(self.nb_elements(), 0);
        elements
    }

    /// Concatenate S m-bit elements into a symbol of E bytes
    fn to_bytes(&self, elements: &[u16]) -> Vec<u8> {
        let m = self.field.m as u32;
        let mut symbol = Vec::with_capacity(self.encoding_symbol_length);
        let (mut acc, mut nb_bits) = (0u32, 0u32);
        for &element in elements {
            acc = (acc << m) | element as u32;
            nb_bits += m;
            while nb_bits >= 8 {
                nb_bits -= 8;
                symbol.push((acc >> nb_bits) as u8);
            }
            acc &= (1 << nb_bits) - 1;
        }
        debug_assert_eq!(symbol.len(), self.encoding_symbol_length);
        symbol
    }

    /// For each row of `coefs`, the symbol sum_i row\[i\] * symbols\[i\]
    ///
    /// Symbols shorter than E are padded with zeros
    fn linear_combinations(&self, symbols: &[&[u8]], coefs: &[Vec<u16>]) -> Vec<Vec<u8>> {
        if self.field.m == 8 {
            // An element is a byte
            return coefs
                .iter()
                .map(|row| {
                    let mut combination = vec![0u8; self.encoding_symbol_length];
                    for (symbol, &c) in symbols.iter().zip(row) {
                        gf256::mul_add(&mut combination, symbol, c as u8);
                    }
                    combination
                })
                .collect();
        }

        let symbols: Vec<Vec<u16>> = symbols.iter().map(|s| self.to_elements(s)).collect();
        coefs
            .iter()
            .map(|row| self.linear_combination(&symbols, row))
            .collect()
    }

    /// sum_i coefs\[i\] * symbols\[i\]
    fn linear_combination(&self, symbols: &[Vec<u16>], coefs: &[u16]) -> Vec<u8> {
        let mut elements = vec![0u16; self.nb_elements()];
        for (symbol, &c) in symbols.iter().zip(coefs) {
            self.field.add_mul(&mut elements, symbol, c);
        }
        self.to_bytes(&elements)
    }
}

impl FecEncoder for RSGalois2MCodec {
    fn encode(&self, data: &[u8]) -> Result<Vec<Box<dyn FecShard>>> {
        let k = self.nb_source_symbols;
        let e = self.encoding_symbol_length;
        if data.len().div_ceil(e) != k {
            return Err(FluteError::new(format!(
                "nb source symbols is {} instead of {}",
                data.len().div_ceil(e),
                k
            )));
        }

        let source: Vec<&[u8]> = data.chunks(e).collect();
        let points: Vec<usize> = (0..k).collect();
        let repair_esis: Vec<usize> = (k..self.nb_encoding_symbols).collect();
        let coefs = self.field.interpolation_matrix(&points, &repair_esis);
        let repairs = self.linear_combinations(&source, &coefs);

        let mut shards: Vec<Box<dyn FecShard>> = Vec::with_capacity(self.nb_encoding_symbols);
        for (esi, &symbol) in source.iter().enumerate() {
            let mut shard = symbol.to_vec();
            shard.resize(e, 0);
            shards.push(Box::new(DataFecShard {
                shard,
                index: esi as u32,
            }));
        }

        for (&esi, shard) in repair_esis.iter().zip(repairs) {
            shards.push(Box::new(DataFecShard {
                shard,
                index: esi as u32,
            }));
        }

        Ok(shards)
    }
}

impl FecDecoder for RSGalois2MCodec {
    fn push_symbol(&mut self, encoding_symbol: &[u8], esi: u32) {
        if self.decode_block.is_some() {
            return;
        }

        // A packet carries G consecutive encoding symbols <https://www.rfc-editor.org/rfc/rfc5510.html#section-4.1>
        let symbols = encoding_symbol
            .chunks(self.encoding_symbol_length)
            .take(self.nb_symbols_per_group);
        for (esi, symbol) in (esi as usize..).zip(symbols) {
            if esi >= self.nb_encoding_symbols || self.received.len() >= self.nb_source_symbols {
                break;
            }
            self.received.entry(esi).or_insert_with(|| symbol.to_vec());
        }
    }

    fn can_decode(&self) -> bool {
        self.received.len() >= self.nb_source_symbols
    }

    fn decode(&mut self) -> bool {
        if self.decode_block.is_some() {
            return true;
        }

        if !self.can_decode() {
            return false;
        }

        let k = self.nb_source_symbols;
        let e = self.encoding_symbol_length;
        let mut block = vec![0u8; k * e];
        for (&esi, symbol) in self.received.range(..k) {
            block[esi * e..esi * e + symbol.len()].copy_from_slice(symbol);
        }

        let missing: Vec<usize> = (0..k)
            .filter(|esi| !self.received.contains_key(esi))
            .collect();
        if !missing.is_empty() {
            let points: Vec<usize> = self.received.keys().copied().collect();
            let values: Vec<&[u8]> = self.received.values().map(Vec::as_slice).collect();
            let coefs = self.field.interpolation_matrix(&points, &missing);
            let symbols = self.linear_combinations(&values, &coefs);
            for (&esi, symbol) in missing.iter().zip(symbols) {
                block[esi * e..(esi + 1) * e].copy_from_slice(&symbol);
            }
        }

        self.received.clear();
        self.decode_block = Some(block);
        true
    }

    fn source_block(&self) -> Result<&[u8]> {
        match self.decode_block.as_ref() {
            Some(e) => Ok(e),
            None => Err(FluteError::new("Block not decoded")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{check_parameters, gf256, GaloisField, RSGalois2MCodec};
    use crate::common::oti::ReedSolomonGF2MSchemeSpecific;
    use crate::fec::{FecDecoder, FecEncoder};

    fn scheme(m: u8, g: u8) -> ReedSolomonGF2MSchemeSpecific {
        ReedSolomonGF2MSchemeSpecific { m, g }
    }

    fn create_data(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i * 31 + i / 7) as u8).collect()
    }

    /// GM = V_{k,k}^-1 * V_{k,n}, computed with Gauss-Jordan elimination (RFC 5510 §8.2.1),
    /// with v_{i,j} = x_j^i, x_0 = 0 and x_j = alpha^(j-1) (Rizzo's codec)
    fn generator_matrix(field: &GaloisField, k: usize, n: usize) -> Vec<Vec<u16>> {
        let v = |i: usize, j: usize| match j {
            0 => (i == 0) as u16,
            _ => field.alpha_pow(i * (j - 1)),
        };
        let mut rows: Vec<Vec<u16>> = (0..k)
            .map(|i| (0..k).map(|j| v(i, j)).chain((0..n).map(|j| v(i, j))).collect())
            .collect();

        for col in 0..k {
            let pivot = (col..k).find(|&r| rows[r][col] != 0).unwrap();
            rows.swap(col, pivot);
            let inv = field.inv(rows[col][col]);
            for x in rows[col].iter_mut() {
                *x = field.mul(*x, inv);
            }
            let pivot_row = rows[col].clone();
            for (r, row) in rows.iter_mut().enumerate() {
                let factor = row[col];
                if r != col && factor != 0 {
                    for (x, &p) in row.iter_mut().zip(&pivot_row) {
                        *x ^= field.mul(factor, p);
                    }
                }
            }
        }

        rows.into_iter().map(|row| row[k..].to_vec()).collect()
    }

    #[test]
    pub fn test_primitive_polynomials() {
        crate::tests::init();
        for m in 2..=16u8 {
            let field = GaloisField::new(m);
            let order = (1usize << m) - 1;
            let mut seen = vec![false; order + 1];
            for i in 0..order {
                let x = field.exp[i] as usize;
                assert!(x != 0 && x <= order && !seen[x], "m={} i={}", m, i);
                seen[x] = true;
                assert_eq!(field.mul(x as u16, field.inv(x as u16)), 1);
            }
        }
    }

    #[test]
    pub fn test_gf256_matches_field() {
        crate::tests::init();
        let field = GaloisField::get(8);
        for a in 0..=255u8 {
            for b in 0..=255u8 {
                assert_eq!(gf256::mul(a, b) as u16, field.mul(a as u16, b as u16));
            }
        }
    }

    #[test]
    pub fn test_check_parameters() {
        crate::tests::init();
        assert!(check_parameters(8, 1, 255, 1).is_ok());
        assert!(check_parameters(16, 1000, 65535, 2).is_ok());
        assert!(check_parameters(3, 4, 7, 3).is_ok());

        assert!(check_parameters(1, 1, 1, 1).is_err());
        assert!(check_parameters(17, 1, 1, 2).is_err());
        assert!(check_parameters(8, 0, 10, 1).is_err());
        assert!(check_parameters(8, 10, 9, 1).is_err());
        assert!(check_parameters(8, 10, 256, 1).is_err());
        assert!(check_parameters(4, 10, 16, 1).is_err());
        assert!(check_parameters(3, 4, 7, 1).is_err());
        assert!(check_parameters(16, 4, 7, 1).is_err());
        assert!(check_parameters(8, 4, 7, 0).is_err());
    }

    #[test]
    pub fn test_elements_packing() {
        crate::tests::init();
        let codec = RSGalois2MCodec::new(1, 1, 3, &scheme(3, 1)).unwrap();
        // 101 001 110 010 111 000 011 100
        let symbol = [0b1010_0111, 0b0010_1110, 0b0001_1100];
        let elements = codec.to_elements(&symbol);
        assert_eq!(elements, vec![5, 1, 6, 2, 7, 0, 3, 4]);
        assert_eq!(codec.to_bytes(&elements), symbol);

        // Padding of a short symbol: 101 001 11|0 ...
        assert_eq!(codec.to_elements(&symbol[..1]), vec![5, 1, 6, 0, 0, 0, 0, 0]);

        let codec = RSGalois2MCodec::new(1, 1, 4, &scheme(16, 1)).unwrap();
        assert_eq!(codec.to_elements(&[0x12, 0x34, 0x56, 0x78]), vec![0x1234, 0x5678]);
    }

    #[test]
    pub fn test_generator_matrix() {
        crate::tests::init();
        for (m, k, n, e) in [(4u8, 5usize, 15usize, 1usize), (8, 6, 12, 2), (16, 4, 9, 2)] {
            let field = GaloisField::get(m);
            let gm = generator_matrix(field, k, n);
            for (i, row) in gm.iter().enumerate() {
                for (j, &x) in row.iter().enumerate().take(k) {
                    assert_eq!(x, (i == j) as u16, "GM is not systematic");
                }
            }

            let data = create_data(k * e);
            let codec = RSGalois2MCodec::new(k, n, e, &scheme(m, 1)).unwrap();
            let shards = codec.encode(&data).unwrap();
            assert_eq!(shards.len(), n);

            let source: Vec<Vec<u16>> = data.chunks(e).map(|s| codec.to_elements(s)).collect();
            for (j, shard) in shards.iter().enumerate() {
                assert_eq!(shard.esi() as usize, j);
                let expected: Vec<u16> = (0..codec.nb_elements())
                    .map(|u| {
                        (0..k).fold(0, |acc, i| acc ^ field.mul(source[i][u], gm[i][j]))
                    })
                    .collect();
                assert_eq!(codec.to_elements(shard.data()), expected, "m={} j={}", m, j);
            }
        }
    }

    fn encode_decode(m: u8, k: usize, n: usize, e: usize, lost: &[usize]) {
        let data = create_data(k * e - e / 2);
        let encoder = RSGalois2MCodec::new(k, n, e, &scheme(m, 1)).unwrap();
        let shards = encoder.encode(&data).unwrap();
        assert_eq!(shards.len(), n);
        assert!(shards.iter().all(|shard| shard.data().len() == e));

        let mut decoder = RSGalois2MCodec::new(k, n, e, &scheme(m, 1)).unwrap();
        for shard in shards.iter().rev() {
            if lost.contains(&(shard.esi() as usize)) {
                continue;
            }
            // The padding of the last source symbol does not need to be sent (RFC 5510 §8.4)
            let symbol = match shard.esi() as usize {
                esi if esi == k - 1 => &shard.data()[..data.len() - esi * e],
                _ => shard.data(),
            };
            assert!(!decoder.can_decode());
            decoder.push_symbol(symbol, shard.esi());
            decoder.push_symbol(symbol, shard.esi());
            if decoder.can_decode() {
                break;
            }
        }

        assert!(decoder.decode(), "m={} k={} n={} lost={:?}", m, k, n, lost);
        let block = decoder.source_block().unwrap();
        assert_eq!(&block[..data.len()], &data[..], "m={} lost={:?}", m, lost);
    }

    #[test]
    pub fn test_encode_decode() {
        crate::tests::init();
        encode_decode(2, 2, 3, 3, &[1]);
        encode_decode(3, 4, 7, 3, &[0, 1, 2]);
        encode_decode(4, 10, 15, 1, &[0, 2, 4, 6, 8]);
        encode_decode(5, 20, 31, 5, &[3, 7, 11, 19]);
        encode_decode(8, 10, 15, 16, &[]);
        encode_decode(8, 10, 15, 16, &[0, 3, 9, 11, 12]);
        encode_decode(8, 200, 255, 4, &(0..55).collect::<Vec<_>>());
        encode_decode(8, 32, 48, 1001, &(5..21).collect::<Vec<_>>());
        encode_decode(12, 5, 9, 3, &[4, 2]);
        encode_decode(16, 20, 30, 10, &(0..10).collect::<Vec<_>>());
        encode_decode(16, 300, 400, 2, &(100..200).collect::<Vec<_>>());
    }

    #[test]
    pub fn test_decode_symbol_groups() {
        crate::tests::init();
        let (k, n, e) = (5, 9, 4);
        let data = create_data(k * e);
        let encoder = RSGalois2MCodec::new(k, n, e, &scheme(8, 1)).unwrap();
        let shards = encoder.encode(&data).unwrap();

        // G = 2, the packets of ESI 0 and 2 are lost
        let mut decoder = RSGalois2MCodec::new(k, n, e, &scheme(8, 2)).unwrap();
        for esi in [4usize, 6, 8] {
            let mut payload = shards[esi].data().to_vec();
            if let Some(next) = shards.get(esi + 1) {
                payload.extend(next.data());
            }
            decoder.push_symbol(&payload, esi as u32);
        }
        assert!(decoder.can_decode());
        assert!(decoder.decode());
        assert_eq!(decoder.source_block().unwrap(), &data[..]);
    }

    /// Repair symbols computed by zfec (m = 8) and OpenFEC (m = 4), see interop/rs/run.sh
    #[test]
    pub fn test_reference_vectors() {
        crate::tests::init();
        let vectors: [(u8, usize, usize, usize, &[&str]); 4] = [
            (8, 4, 7, 2, &["5621", "1dc3", "96ed"]),
            (
                8,
                10,
                15,
                16,
                &[
                    "ed8e72239d7791f3634619c39c0d8d2a",
                    "386b1f920849fb3fb97cc893a7cea169",
                    "aa413687673e8e2a1c94e35de3f16930",
                    "b0f4eeaf2353535d7c6fecb74a0f6b2d",
                    "dcfe3e93a8a1a391b73f6aedca9c320f",
                ],
            ),
            (4, 4, 7, 2, &["a126", "2a91", "9a32"]),
            (
                4,
                10,
                15,
                8,
                &[
                    "b37bc624b80f84f2",
                    "13bccc9cbe344f8e",
                    "aaccb955c3f05934",
                    "5d7d17bbdbe4dd0a",
                    "3f8e92dc0238f8a1",
                ],
            ),
        ];

        for (m, k, n, e, repairs) in vectors {
            let codec = RSGalois2MCodec::new(k, n, e, &scheme(m, 1)).unwrap();
            let shards = codec.encode(&create_data(k * e)).unwrap();
            assert_eq!(shards.len() - k, repairs.len());
            for (shard, repair) in shards[k..].iter().zip(repairs) {
                assert_eq!(shard.data(), &from_hex(repair)[..], "m={} k={} ESI={}", m, k, shard.esi());
            }
        }
    }

    fn from_hex(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    /// Repair symbols computed by zfec and OpenFEC, generated by interop/rs/run.sh
    #[test]
    #[ignore = "requires the vectors generated by interop/rs/run.sh"]
    pub fn test_interop_vectors() {
        crate::tests::init();
        let path = std::env::var("FLUTE_RS_INTEROP_VECTORS")
            .expect("FLUTE_RS_INTEROP_VECTORS is not set, run interop/rs/run.sh");
        let content = std::fs::read_to_string(path).unwrap();

        // (codec, m, k, n, E) -> [(ESI, repair symbol)]
        type Case<'a> = (&'a str, u8, usize, usize, usize);
        let mut cases: BTreeMap<Case, Vec<(usize, Vec<u8>)>> = BTreeMap::new();
        for line in content.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(fields.len(), 7, "invalid line {}", line);
            let case = (
                fields[0],
                fields[1].parse().unwrap(),
                fields[2].parse().unwrap(),
                fields[3].parse().unwrap(),
                fields[4].parse().unwrap(),
            );
            let repair = (fields[5].parse().unwrap(), from_hex(fields[6]));
            cases.entry(case).or_default().push(repair);
        }
        assert!(!cases.is_empty());

        for (&(codec, m, k, n, e), repairs) in &cases {
            assert_eq!(repairs.len(), n - k);
            let data = create_data(k * e);
            let encoder = RSGalois2MCodec::new(k, n, e, &scheme(m, 1)).unwrap();
            let shards = encoder.encode(&data).unwrap();
            for (esi, repair) in repairs {
                assert_eq!(
                    shards[*esi].data(),
                    &repair[..],
                    "{} m={} k={} n={} E={} ESI={}",
                    codec,
                    m,
                    k,
                    n,
                    e,
                    esi
                );
            }

            // Decode with the repair symbols of the reference implementation
            // in place of the first source symbols
            let mut decoder = RSGalois2MCodec::new(k, n, e, &scheme(m, 1)).unwrap();
            let nb_lost = repairs.len().min(k);
            for (esi, repair) in &repairs[..nb_lost] {
                decoder.push_symbol(repair, *esi as u32);
            }
            for esi in nb_lost..k {
                decoder.push_symbol(&data[esi * e..(esi + 1) * e], esi as u32);
            }
            assert!(decoder.decode());
            assert_eq!(decoder.source_block().unwrap(), &data[..], "{} m={} k={}", codec, m, k);
        }
        println!("{} cases compatible with zfec and OpenFEC", cases.len());
    }

    #[test]
    pub fn test_decode_ignores_out_of_range_esi() {
        crate::tests::init();
        let mut decoder = RSGalois2MCodec::new(2, 3, 1, &scheme(8, 1)).unwrap();
        decoder.push_symbol(&[1], 3);
        decoder.push_symbol(&[1], u32::MAX);
        decoder.push_symbol(&[1], 0);
        assert!(!decoder.can_decode());
        assert!(!decoder.decode());
    }
}

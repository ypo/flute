///
/// Block Partitioning Algorithm  
/// See <https://www.rfc-editor.org/rfc/rfc5052#section-9.1>
///
///
/// This function implements the block partitioning algorithm as defined in RFC 5052.
/// The algorithm is used to partition a large amount of data into smaller blocks that can be transmitted or encoded more efficiently.
///
/// # Arguments
///
///    * b: Maximum Source Block Length, i.e., the maximum number of source symbols per source block.  
///
///    * l: Transfer Length in octets.  
///
///    * e: Encoding Symbol Length in octets.  
///
/// # Returns
///
/// The function returns a tuple of four values:  
///     * a_large: The length of each of the larger source blocks in symbols.  
///     * a_small: The length of each of the smaller source blocks in symbols.  
///     * nb_a_large: The number of blocks composed of a_large symbols.  
///     * nb_blocks: The total number of blocks.  
///
pub fn block_partitioning(b: u64, l: u64, e: u64) -> (u64, u64, u64, u64) {
    if b == 0 {
        log::warn!("Maximum Source Block Length is 0");
        return (0, 0, 0, 0);
    }

    if e == 0 {
        log::error!("Encoding Symbol Length is 0");
        return (0, 0, 0, 0);
    }

    let t = num_integer::div_ceil(l, e);
    let n = num_integer::div_ceil(t, b);
    log::debug!("t={} n={} b={} l={} e={}", t, n, b, l, e);
    partition(t, n)
}

/// Partition\[I, J\]: splits `t` source symbols into `n` source blocks
/// See <https://www.rfc-editor.org/rfc/rfc6330.html#section-4.4.1.2>
///
/// Raptor (RFC 5053) and RaptorQ (RFC 6330) announce the number of source blocks Z,
/// the object is partitioned with Partition\[Kt, Z\].
/// Source blocks without any symbol (`n` > `t`) are not counted.
///
/// Returns the same tuple as [`block_partitioning`]
///
pub fn partition(t: u64, n: u64) -> (u64, u64, u64, u64) {
    let n = n.min(t);
    if n == 0 {
        return (0, 0, 0, 0);
    }

    let a_large = num_integer::div_ceil(t, n);
    let a_small = num_integer::div_floor(t, n);
    let nb_a_large = t - (a_small * n);
    let nb_blocks = n;

    (a_large, a_small, nb_a_large, nb_blocks)
}

/// Calculates the size of a block in octets.
///
/// # Arguments
///
/// * `a_large`: The length of each of the larger source blocks in symbols.
/// * `a_small`: The length of each of the smaller source blocks in symbols.
/// * `nb_a_large`: The number of blocks composed of `a_large` symbols.
/// * `l`: Transfer length in octets.
/// * `e`: Encoding symbol length in octets.
/// * `sbn`: Source block number.
///
/// # Returns
///
/// The size of the block in octets, or `None` if `sbn` is outside the partition.
///
pub fn block_length(
    a_large: u64,
    a_small: u64,
    nb_a_large: u64,
    l: u64,
    e: u64,
    sbn: u32,
) -> Option<u64> {
    let sbn = sbn as u64;

    let large_block_size = a_large.checked_mul(e)?;
    let small_block_size = a_small.checked_mul(e)?;

    let (offset, block_size) = if sbn < nb_a_large {
        (sbn.checked_mul(large_block_size)?, large_block_size)
    } else {
        let small_offset = (sbn - nb_a_large).checked_mul(small_block_size)?;
        let offset = nb_a_large
            .checked_mul(large_block_size)?
            .checked_add(small_offset)?;
        (offset, small_block_size)
    };

    if offset >= l || block_size == 0 {
        return None;
    }

    Some(block_size.min(l - offset))
}

#[cfg(test)]
mod tests {

    #[test]
    pub fn partition_empty_file() {
        crate::tests::init();
        let (a_large, a_small, nb_a_large, nb_blocks) = super::block_partitioning(64, 0, 1024);
        log::info!(
            "a_large={} a_small={} nb_a_large={} nb_blocks={}",
            a_large,
            a_small,
            nb_a_large,
            nb_blocks
        );
        assert!(nb_blocks == 0);
    }

    #[test]
    pub fn partition_into_announced_blocks() {
        crate::tests::init();
        // Partition[Kt=10, Z=6]: 4 blocks of 2 symbols then 2 blocks of 1 symbol
        assert_eq!(super::partition(10, 6), (2, 1, 4, 6));
        assert_eq!(super::partition(30, 7), (5, 4, 2, 7));
        // Blocks without symbol are not counted
        assert_eq!(super::partition(3, 5), (1, 1, 0, 3));
        assert_eq!(super::partition(0, 5), (0, 0, 0, 0));
        assert_eq!(super::partition(5, 0), (0, 0, 0, 0));
    }

    #[test]
    pub fn block_length_covers_transfer_length() {
        crate::tests::init();
        for (b, l, e) in [
            (64, 1, 1024),
            (64, 1024, 1024),
            (64, 1025, 1024),
            (5, 1000, 100),
            (7, 100_000, 100),
            (3, 1_000_001, 1400),
            (u32::MAX as u64, 100, 100),
        ] {
            let (a_large, a_small, nb_a_large, nb_blocks) = super::block_partitioning(b, l, e);
            let mut total = 0;
            for sbn in 0..nb_blocks as u32 {
                let len = super::block_length(a_large, a_small, nb_a_large, l, e, sbn).unwrap();
                assert!(len > 0 && len <= a_large * e);
                total += len;
            }
            assert_eq!(total, l, "b={b} l={l} e={e}");

            // SBN outside the partition
            for sbn in [nb_blocks as u32, nb_blocks as u32 + 1, u32::MAX] {
                assert_eq!(
                    super::block_length(a_large, a_small, nb_a_large, l, e, sbn),
                    None
                );
            }
        }
    }
}

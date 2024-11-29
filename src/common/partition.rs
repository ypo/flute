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
/// The size of the block in octets.
///
pub fn block_length(a_large: u64, a_small: u64, nb_a_large: u64, l: u64, e: u64, sbn: u32) -> u64 {
    let sbn = sbn as u64;

    let large_block_size = a_large * e;
    let small_block_size = a_small * e;

    if sbn + 1 < nb_a_large {
        return large_block_size;
    }

    if sbn + 1 == nb_a_large {
        let large_size = nb_a_large * large_block_size;
        if large_size <= l {
            return large_block_size;
        }

        // Should never happen ?
        return l - ((nb_a_large - 1) * large_block_size);
    }

    let l = l - (nb_a_large * large_block_size);
    let sbn = sbn - nb_a_large;
    let small_size = (sbn + 1) * small_block_size;
    if small_size <= l {
        return small_block_size;
    }

    l - (sbn * small_block_size)
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
}

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
    let t = num_integer::div_ceil(l, e);
    let mut n = num_integer::div_ceil(t, b);
    if n == 0 {
        n = 1
    }

    let a_large = num_integer::div_ceil(t, n);
    let a_small = num_integer::div_floor(t, n);
    let nb_a_large = t - (a_small * n);
    let nb_blocks = n;

    (a_large, a_small, nb_a_large, nb_blocks)
}

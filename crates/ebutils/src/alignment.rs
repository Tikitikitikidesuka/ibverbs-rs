/// Returns the smallest multiple of `alignment` that is greater than or equal to `n`.
///
/// If either argument is `0`, the function returns `n` unchanged.
///
/// If `alignment` is a power of two, this delegates to [`align_up_pow2`].
///
/// # Examples
///
/// ```
/// # use ebutils::align_up;
/// assert_eq!(align_up(10, 4), 12);
/// assert_eq!(align_up(8, 4), 8);
/// assert_eq!(align_up(0, 4), 0);
/// ```
pub fn align_up(n: usize, alignment: usize) -> usize {
    if n == 0 || alignment == 0 {
        return n;
    }

    if let Some(_) = pow2_exponent(alignment) {
        align_up_pow2(
            n,
            alignment
                .trailing_zeros()
                .try_into()
                .expect("bits fit into u8"),
        )
    } else {
        let remainder = n % alignment;
        if remainder == 0 {
            n // Already aligned
        } else {
            n + alignment - remainder
        }
    }
}


/// Returns the largest multiple of `alignment` that is less than or equal to `n`.
///
/// If either argument is `0`, the function returns `n` unchanged.
///
/// If `alignment` is a power of two, this delegates to [`align_down_pow2`].
///
/// # Examples
///
/// ```
/// # use ebutils::align_down;
/// assert_eq!(align_down(10, 4), 8);
/// assert_eq!(align_down(8, 4), 8);
/// assert_eq!(align_down(0, 4), 0);
/// ```
pub fn align_down(n: usize, alignment: usize) -> usize {
    if n == 0 || alignment == 0 {
        return n;
    }

    if let Some(_) = pow2_exponent(alignment) {
        align_down_pow2(
            n,
            alignment
                .trailing_zeros()
                .try_into()
                .expect("bits fit into u8"),
        )
    } else {
        // Integer division naturally rounds down
        (n / alignment) * alignment
    }
}

/// Returns the smallest multiple of `2^exponent` that is greater than or equal to `n`.
///
/// This function assumes `exponent < usize::BITS`.
///
/// # Examples
///
/// ```
/// # use ebutils::align_up_pow2;
/// assert_eq!(align_up_pow2(10, 2), 12); // align to 4
/// assert_eq!(align_up_pow2(8, 3), 8);  // align to 8
/// ```
pub fn align_up_pow2(n: usize, exponent: u8) -> usize {
    // Step 1: Calculate alignment value (2^exponent)
    // Example: for exponent=3, alignment=8 (binary 1000)
    let alignment = 1 << exponent;

    // Step 2: Create a bit mask with all bits below the alignment bit set to 1
    // Example: for alignment=8, mask=7 (binary 0111)
    let mask = alignment - 1;

    // Step 3: Add the mask to the size to ensure we reach at least the next multiple
    // - If size is already aligned: we'll exceed it slightly but not reach the next multiple
    // - If size is not aligned: this pushes us to at least the next multiple
    // Example: for size=10, mask=3 (alignment=4), size+mask=13
    let size_plus_mask = n + mask;

    // Step 4: Create an inverted mask that has 1s in all positions where
    // we want to keep bits (alignment bit and higher)
    // Example: for mask=3 (binary 0011), inverted_mask=~3 (binary ...1111100)
    let inverted_mask = !mask;

    // Step 5: Apply the inverted mask to zero out all bits below the alignment bit
    // This effectively rounds down to the nearest multiple of alignment
    // But since we added the mask in step 3, we're actually rounding up
    // Example: for size_plus_mask=13 (binary 1101), inverted_mask=...1111100
    //          13 & ...1111100 = 12 (binary 1100)

    size_plus_mask & inverted_mask
}


/// Returns the largest multiple of `2^exponent` that is less than or equal to `n`.
///
/// # Examples
///
/// ```
/// # use ebutils::align_down_pow2;
/// assert_eq!(align_down_pow2(10, 3), 8); // align to 8
/// assert_eq!(align_down_pow2(8, 3), 8);
/// ```
pub fn align_down_pow2(n: usize, exponent: u8) -> usize {
    // Calculate the alignment value (2^exponent)
    // Example: for exponent=3, alignment=8 (binary 1000)
    let alignment = 1 << exponent;

    // Create a mask with all bits below the alignment bit set to 1
    // Example: for alignment=8, mask=7 (binary 0111)
    let mask = alignment - 1;

    // Create an inverted mask that has 1s in all positions where
    // we want to keep bits (alignment bit and higher)
    // Example: for mask=7 (binary 0111), inverted_mask=~7 (binary ...1111000)
    let inverted_mask = !mask;

    // Apply the inverted mask to zero out all bits below the alignment bit
    // This effectively rounds down to the nearest multiple of alignment
    // Example: for size=10 (binary 1010), inverted_mask=...1111000
    //          10 & ...1111000 = 8 (binary 1000)

    n & inverted_mask
}

/// Returns `true` if `n` is a multiple of `alignment`.
///
/// If either argument is `0`, the function returns `true`.
///
/// If `alignment` is a power of two, this delegates to [`check_alignment_pow2`].
///
/// # Examples
///
/// ```
/// # use ebutils::check_alignment;
/// assert!(check_alignment(16, 4));
/// assert!(!check_alignment(18, 4));
/// ```
pub fn check_alignment(n: usize, alignment: usize) -> bool {
    if n == 0 || alignment == 0 {
        return true;
    }

    if let Some(_) = pow2_exponent(alignment) {
        check_alignment_pow2(
            n,
            alignment
                .trailing_zeros()
                .try_into()
                .expect("bits fit into u8"),
        )
    } else {
        n.is_multiple_of(alignment)
    }
}

/// Returns `true` if `size` is a multiple of `2^exponent`.
///
/// # Examples
///
/// ```
/// # use ebutils::check_alignment_pow2;
/// assert!(check_alignment_pow2(16, 4)); // 16 is aligned to 16
/// assert!(!check_alignment_pow2(18, 4));
/// ```
pub fn check_alignment_pow2(size: usize, exponent: u8) -> bool {
    // Calculate the alignment value (2^exponent)
    let alignment = 1 << exponent;

    // Create a mask with all bits below the alignment bit set to 1
    // Example: for alignment=8 (binary 1000), mask=7 (binary 0111)
    let mask = alignment - 1;

    // Check if the size has any bits set in positions below the alignment bit
    // If (size & mask) is 0, then size is divisible by alignment (properly aligned)
    (size & mask) == 0
}

/// Returns `n` wrapped modulo `wrap`.
///
/// If `wrap` is `0`, returns `n`.
///
/// If `wrap` is a power of two, this delegates to [`wrap_around_pow2`].
///
/// # Examples
///
/// ```
/// # use ebutils::wrap_around;
/// assert_eq!(wrap_around(10, 4), 2);
/// assert_eq!(wrap_around(10, 0), 10);
/// ```
pub fn wrap_around(n: usize, wrap: usize) -> usize {
    if wrap == 0 {
        return n;
    }

    if let Some(_) = pow2_exponent(wrap) {
        wrap_around_pow2(
            n,
            wrap.trailing_zeros().try_into().expect("bits fit into u8"),
        )
    } else {
        n % wrap
    }
}

/// Wraps `n` with modulus `2^wrap_pow2`.
///
/// # Examples
///
/// ```
/// # use ebutils::wrap_around_pow2;
/// assert_eq!(wrap_around_pow2(10, 2), 2); // wrap by 4
/// ```
pub fn wrap_around_pow2(n: usize, wrap_pow2: u8) -> usize {
    // Calculate the wrap value (2^exponent)
    // Example: for exponent=3, alignment=8 (binary 1000)
    let wrap = 1 << wrap_pow2;

    // Create a mask with all bits below the wrap bit set to 1
    // Example: for wrap=8, mask=7 (binary 0111)
    let mask = wrap - 1;

    // Apply the mask to keep only the lower bits, effectively wrapping around
    // Example: for n=10 (binary 1010), wrap=8, mask=7 (binary 0111)
    //          10 & 7 = 2 (binary 0010)
    //          This wraps 10 around the range [0, 8) to get 2
    n & mask
}

/// Returns `2^exponent`.
///
/// # Examples
///
/// ```
/// # use ebutils::pow2;
/// assert_eq!(pow2(3), 8);
/// ```
pub fn pow2(exponent: u8) -> usize {
    1 << exponent
}

/// Returns `Some(exponent)` if `n` is a power of two, otherwise `None`.
///
/// # Examples
///
/// ```
/// # use ebutils::pow2_exponent;
/// assert_eq!(pow2_exponent(8), Some(3));
/// assert_eq!(pow2_exponent(10), None);
/// ```
pub fn pow2_exponent(n: usize) -> Option<u8> {
    if n != 0 && (n & (n - 1)) == 0 {
        Some(n.trailing_zeros().try_into().expect("bits fit into u8"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_up() {
        // Edge cases
        assert_eq!(align_up(0, 4), 0);
        assert_eq!(align_up(42, 0), 42);

        // Power of two alignments
        assert_eq!(align_up(1, 4), 4);
        assert_eq!(align_up(3, 4), 4);
        assert_eq!(align_up(4, 4), 4);
        assert_eq!(align_up(5, 4), 8);
        assert_eq!(align_up(7, 8), 8);
        assert_eq!(align_up(8, 8), 8);
        assert_eq!(align_up(9, 8), 16);
        assert_eq!(align_up(1023, 1024), 1024);
        assert_eq!(align_up(1025, 1024), 2048);

        // Non-power of two alignments
        assert_eq!(align_up(1, 3), 3);
        assert_eq!(align_up(3, 3), 3);
        assert_eq!(align_up(4, 3), 6);
        assert_eq!(align_up(5, 3), 6);
        assert_eq!(align_up(7, 6), 12);
        assert_eq!(align_up(12, 6), 12);
        assert_eq!(align_up(13, 6), 18);
    }

    #[test]
    fn test_align_down() {
        // Edge cases
        assert_eq!(align_down(0, 4), 0);
        assert_eq!(align_down(42, 0), 42);

        // Power of two alignments
        assert_eq!(align_down(1, 4), 0);
        assert_eq!(align_down(3, 4), 0);
        assert_eq!(align_down(4, 4), 4);
        assert_eq!(align_down(5, 4), 4);
        assert_eq!(align_down(7, 8), 0);
        assert_eq!(align_down(8, 8), 8);
        assert_eq!(align_down(9, 8), 8);
        assert_eq!(align_down(1023, 1024), 0);
        assert_eq!(align_down(1025, 1024), 1024);

        // Non-power of two alignments
        assert_eq!(align_down(1, 3), 0);
        assert_eq!(align_down(3, 3), 3);
        assert_eq!(align_down(4, 3), 3);
        assert_eq!(align_down(5, 3), 3);
        assert_eq!(align_down(7, 6), 6);
        assert_eq!(align_down(12, 6), 12);
        assert_eq!(align_down(13, 6), 12);
    }

    #[test]
    fn test_align_up_pow2() {
        assert_eq!(align_up_pow2(0, 2), 0);
        assert_eq!(align_up_pow2(1, 2), 4);
        assert_eq!(align_up_pow2(2, 2), 4);
        assert_eq!(align_up_pow2(3, 2), 4);
        assert_eq!(align_up_pow2(4, 2), 4);
        assert_eq!(align_up_pow2(5, 2), 8);
        assert_eq!(align_up_pow2(6, 2), 8);
        assert_eq!(align_up_pow2(7, 2), 8);
        assert_eq!(align_up_pow2(8, 2), 8);

        assert_eq!(align_up_pow2(1, 3), 8);
        assert_eq!(align_up_pow2(7, 3), 8);
        assert_eq!(align_up_pow2(8, 3), 8);
        assert_eq!(align_up_pow2(9, 3), 16);
        assert_eq!(align_up_pow2(15, 3), 16);
        assert_eq!(align_up_pow2(16, 3), 16);

        assert_eq!(align_up_pow2(4095, 12), 4096);
        assert_eq!(align_up_pow2(4096, 12), 4096);
        assert_eq!(align_up_pow2(4097, 12), 8192);
    }

    #[test]
    fn test_align_down_pow2() {
        assert_eq!(align_down_pow2(0, 2), 0);
        assert_eq!(align_down_pow2(1, 2), 0);
        assert_eq!(align_down_pow2(2, 2), 0);
        assert_eq!(align_down_pow2(3, 2), 0);
        assert_eq!(align_down_pow2(4, 2), 4);
        assert_eq!(align_down_pow2(5, 2), 4);

        assert_eq!(align_down_pow2(7, 3), 0);
        assert_eq!(align_down_pow2(8, 3), 8);
        assert_eq!(align_down_pow2(15, 3), 8);

        assert_eq!(align_down_pow2(4095, 12), 0);
        assert_eq!(align_down_pow2(4096, 12), 4096);
        assert_eq!(align_down_pow2(8191, 12), 4096);
    }

    #[test]
    fn test_check_alignment() {
        // Edge cases
        assert!(check_alignment(0, 4));
        assert!(check_alignment(42, 0));

        // Power of two alignments
        assert!(!check_alignment(1, 4));
        assert!(!check_alignment(3, 4));
        assert!(check_alignment(4, 4));
        assert!(!check_alignment(5, 4));
        assert!(!check_alignment(7, 8));
        assert!(check_alignment(8, 8));
        assert!(!check_alignment(9, 8));
        assert!(!check_alignment(1023, 1024));
        assert!(check_alignment(1024, 1024));

        // Non-power of two alignments
        assert!(!check_alignment(1, 3));
        assert!(check_alignment(3, 3));
        assert!(!check_alignment(4, 3));
        assert!(!check_alignment(5, 3));
        assert!(check_alignment(6, 3));
        assert!(!check_alignment(7, 6));
        assert!(check_alignment(12, 6));
        assert!(!check_alignment(13, 6));
    }

    #[test]
    fn test_check_alignment_pow2() {
        assert!(check_alignment_pow2(0, 2));
        assert!(!check_alignment_pow2(1, 2));
        assert!(!check_alignment_pow2(2, 2));
        assert!(!check_alignment_pow2(3, 2));
        assert!(check_alignment_pow2(4, 2));
        assert!(!check_alignment_pow2(5, 2));

        assert!(!check_alignment_pow2(7, 3));
        assert!(check_alignment_pow2(8, 3));
        assert!(!check_alignment_pow2(9, 3));

        assert!(!check_alignment_pow2(4095, 12));
        assert!(check_alignment_pow2(4096, 12));
        assert!(!check_alignment_pow2(4097, 12));
    }

    #[test]
    fn test_wrap_around() {
        // Edge cases
        assert_eq!(wrap_around(42, 0), 42);

        // Power of two wraps
        assert_eq!(wrap_around(0, 4), 0);
        assert_eq!(wrap_around(1, 4), 1);
        assert_eq!(wrap_around(3, 4), 3);
        assert_eq!(wrap_around(4, 4), 0);
        assert_eq!(wrap_around(5, 4), 1);
        assert_eq!(wrap_around(7, 8), 7);
        assert_eq!(wrap_around(8, 8), 0);
        assert_eq!(wrap_around(9, 8), 1);
        assert_eq!(wrap_around(15, 8), 7);
        assert_eq!(wrap_around(16, 8), 0);
        assert_eq!(wrap_around(1023, 1024), 1023);
        assert_eq!(wrap_around(1024, 1024), 0);
        assert_eq!(wrap_around(1025, 1024), 1);

        // Non-power of two wraps
        assert_eq!(wrap_around(0, 3), 0);
        assert_eq!(wrap_around(1, 3), 1);
        assert_eq!(wrap_around(2, 3), 2);
        assert_eq!(wrap_around(3, 3), 0);
        assert_eq!(wrap_around(4, 3), 1);
        assert_eq!(wrap_around(5, 3), 2);
        assert_eq!(wrap_around(6, 3), 0);
        assert_eq!(wrap_around(7, 6), 1);
        assert_eq!(wrap_around(12, 6), 0);
        assert_eq!(wrap_around(13, 6), 1);
    }

    #[test]
    fn test_wrap_around_pow2() {
        assert_eq!(wrap_around_pow2(0, 2), 0);
        assert_eq!(wrap_around_pow2(1, 2), 1);
        assert_eq!(wrap_around_pow2(2, 2), 2);
        assert_eq!(wrap_around_pow2(3, 2), 3);
        assert_eq!(wrap_around_pow2(4, 2), 0);
        assert_eq!(wrap_around_pow2(5, 2), 1);
        assert_eq!(wrap_around_pow2(7, 2), 3);
        assert_eq!(wrap_around_pow2(8, 2), 0);

        assert_eq!(wrap_around_pow2(7, 3), 7);
        assert_eq!(wrap_around_pow2(8, 3), 0);
        assert_eq!(wrap_around_pow2(9, 3), 1);
        assert_eq!(wrap_around_pow2(15, 3), 7);
        assert_eq!(wrap_around_pow2(16, 3), 0);

        assert_eq!(wrap_around_pow2(4095, 12), 4095);
        assert_eq!(wrap_around_pow2(4096, 12), 0);
        assert_eq!(wrap_around_pow2(4097, 12), 1);
    }

    #[test]
    fn test_pow2() {
        assert_eq!(pow2(0), 1);
        assert_eq!(pow2(1), 2);
        assert_eq!(pow2(2), 4);
        assert_eq!(pow2(3), 8);
        assert_eq!(pow2(4), 16);
        assert_eq!(pow2(5), 32);
        assert_eq!(pow2(6), 64);
        assert_eq!(pow2(7), 128);
        assert_eq!(pow2(8), 256);
        assert_eq!(pow2(10), 1024);
        assert_eq!(pow2(12), 4096);
        assert_eq!(pow2(20), 1048576);
        assert_eq!(pow2(30), 1073741824);
    }

    #[test]
    fn test_is_pow2() {
        // Test powers of 2 - should return Yes with the correct exponent
        assert!(matches!(pow2_exponent(1), Some(0))); // 2^0 = 1
        assert!(matches!(pow2_exponent(2), Some(1))); // 2^1 = 2
        assert!(matches!(pow2_exponent(4), Some(2))); // 2^2 = 4
        assert!(matches!(pow2_exponent(8), Some(3))); // 2^3 = 8
        assert!(matches!(pow2_exponent(16), Some(4))); // 2^4 = 16
        assert!(matches!(pow2_exponent(32), Some(5))); // 2^5 = 32
        assert!(matches!(pow2_exponent(64), Some(6))); // 2^6 = 64
        assert!(matches!(pow2_exponent(128), Some(7))); // 2^7 = 128
        assert!(matches!(pow2_exponent(256), Some(8))); // 2^8 = 256
        assert!(matches!(pow2_exponent(1 << 30), Some(30))); // 2^30

        // Test non-powers of 2 - should return No
        assert!(matches!(pow2_exponent(0), None));
        assert!(matches!(pow2_exponent(3), None));
        assert!(matches!(pow2_exponent(5), None));
        assert!(matches!(pow2_exponent(6), None));
        assert!(matches!(pow2_exponent(7), None));
        assert!(matches!(pow2_exponent(9), None));
        assert!(matches!(pow2_exponent(10), None));
        assert!(matches!(pow2_exponent(15), None));
        assert!(matches!(pow2_exponent(127), None));
    }
}

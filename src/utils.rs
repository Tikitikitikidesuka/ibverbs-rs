pub fn align_up(n: usize, alignment: usize) -> usize {
    if n == 0 || alignment == 0 {
        return n;
    }

    if is_power_of_two(alignment) {
        align_up_2pow(n, alignment.trailing_zeros() as u8)
    } else {
        let remainder = n % alignment;
        if remainder == 0 {
            n // Already aligned
        } else {
            n + alignment - remainder
        }
    }
}

pub fn align_down(n: usize, alignment: usize) -> usize {
    if n == 0 || alignment == 0 {
        return n;
    }

    if is_power_of_two(alignment) {
        align_down_2pow(n, alignment.trailing_zeros() as u8)
    } else {
        // Integer division naturally rounds down
        (n / alignment) * alignment
    }
}

pub fn align_up_2pow(n: usize, exponent: u8) -> usize {
    // Step 1: Calculate alignment value (2^exponent)
    // Example: for exponent=3, alignment=8 (binary 1000)
    let alignment = 1 << exponent;

    // Step 2: Create a bit mask with all bits below the alignment bit set to 1
    // Example: for alignment=8, mask=7 (binary 0111)
    let mask = alignment - 1;

    // Step 3: Add the mask to the size to ensure we reach at least the next multiple
    // - If size is already aligned: we'll exceed it slightly but not reach next multiple
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

pub fn align_down_2pow(n: usize, exponent: u8) -> usize {
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

pub fn check_alignment(n: usize, alignment: usize) -> bool {
    if n == 0 || alignment == 0 {
        return true;
    }

    if is_power_of_two(alignment) {
        check_alignment_2pow(n, alignment.trailing_zeros() as u8)
    } else {
        n % alignment == 0
    }
}

pub fn check_alignment_2pow(size: usize, exponent: u8) -> bool {
    // Calculate the alignment value (2^exponent)
    let alignment = 1 << exponent;

    // Create a mask with all bits below the alignment bit set to 1
    // Example: for alignment=8 (binary 1000), mask=7 (binary 0111)
    let mask = alignment - 1;

    // Check if the size has any bits set in positions below the alignment bit
    // If (size & mask) is 0, then size is divisible by alignment (properly aligned)
    (size & mask) == 0
}

pub fn is_power_of_two(n: usize) -> bool {
    n != 0 && (n & (n - 1)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_power_of_two() {
        assert!(is_power_of_two(1));
        assert!(is_power_of_two(2));
        assert!(is_power_of_two(4));
        assert!(is_power_of_two(8));
        assert!(is_power_of_two(16));
        assert!(is_power_of_two(32));
        assert!(is_power_of_two(64));
        assert!(is_power_of_two(128));
        assert!(is_power_of_two(256));
        assert!(is_power_of_two(1 << 30));

        assert!(!is_power_of_two(0));
        assert!(!is_power_of_two(3));
        assert!(!is_power_of_two(5));
        assert!(!is_power_of_two(6));
        assert!(!is_power_of_two(7));
        assert!(!is_power_of_two(9));
        assert!(!is_power_of_two(10));
        assert!(!is_power_of_two(15));
        assert!(!is_power_of_two(127));
    }

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
    fn test_align_up_2pow() {
        assert_eq!(align_up_2pow(0, 2), 0);
        assert_eq!(align_up_2pow(1, 2), 4);
        assert_eq!(align_up_2pow(2, 2), 4);
        assert_eq!(align_up_2pow(3, 2), 4);
        assert_eq!(align_up_2pow(4, 2), 4);
        assert_eq!(align_up_2pow(5, 2), 8);

        assert_eq!(align_up_2pow(4, 3), 8);
        assert_eq!(align_up_2pow(8, 3), 8);
        assert_eq!(align_up_2pow(9, 3), 16);

        assert_eq!(align_up_2pow(4095, 12), 4096);
        assert_eq!(align_up_2pow(4096, 12), 4096);
        assert_eq!(align_up_2pow(4097, 12), 8192);
    }

    #[test]
    fn test_align_down_2pow() {
        assert_eq!(align_down_2pow(0, 2), 0);
        assert_eq!(align_down_2pow(1, 2), 0);
        assert_eq!(align_down_2pow(2, 2), 0);
        assert_eq!(align_down_2pow(3, 2), 0);
        assert_eq!(align_down_2pow(4, 2), 4);
        assert_eq!(align_down_2pow(5, 2), 4);

        assert_eq!(align_down_2pow(7, 3), 0);
        assert_eq!(align_down_2pow(8, 3), 8);
        assert_eq!(align_down_2pow(15, 3), 8);

        assert_eq!(align_down_2pow(4095, 12), 0);
        assert_eq!(align_down_2pow(4096, 12), 4096);
        assert_eq!(align_down_2pow(8191, 12), 4096);
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
    fn test_check_alignment_2pow() {
        assert!(check_alignment_2pow(0, 2));
        assert!(!check_alignment_2pow(1, 2));
        assert!(!check_alignment_2pow(2, 2));
        assert!(!check_alignment_2pow(3, 2));
        assert!(check_alignment_2pow(4, 2));
        assert!(!check_alignment_2pow(5, 2));

        assert!(!check_alignment_2pow(7, 3));
        assert!(check_alignment_2pow(8, 3));
        assert!(!check_alignment_2pow(9, 3));

        assert!(!check_alignment_2pow(4095, 12));
        assert!(check_alignment_2pow(4096, 12));
        assert!(!check_alignment_2pow(4097, 12));
    }
}

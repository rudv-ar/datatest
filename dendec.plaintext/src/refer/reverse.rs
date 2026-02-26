/// refer/reverse.rs — Reverse complement utility
///
/// A pure function with no dependencies. Used during encode (strand selection)
/// and decode (recovering the original 8-mer when the BED strand is -).
///
/// The fixed mapping is biological convention:
///   A ↔ T   (adenine pairs with thymine)
///   G ↔ C   (guanine pairs with cytosine)
/// The sequence is then reversed to give the 5'→3' complement strand.

/// Compute the reverse complement of an 8-mer byte slice.
/// Operates on uppercase A/T/G/C bytes only.
/// This is again only for future purposes. Used for checking through the complement base sequence
/// also, dead code 
#[allow(dead_code)]
pub fn reverse_complement(kmer: &[u8]) -> [u8; 8] {
    let mut rc = [0u8; 8];
    for (i, &b) in kmer.iter().rev().enumerate() {
        rc[i] = match b {
            b'A' => b'T',
            b'T' => b'A',
            b'G' => b'C',
            b'C' => b'G',
            x    => x,
        };
    }
    rc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        assert_eq!(reverse_complement(b"ATGCGATC"), *b"GATCGCAT");
    }

    #[test]
    fn test_palindrome() {
        // A palindromic 8-mer is its own reverse complement
        assert_eq!(reverse_complement(b"AATTAATT"), *b"AATTAATT");
    }

    #[test]
    fn test_all_same_base() {
        assert_eq!(reverse_complement(b"AAAAAAAA"), *b"TTTTTTTT");
        assert_eq!(reverse_complement(b"CCCCCCCC"), *b"GGGGGGGG");
        assert_eq!(reverse_complement(b"TTTTTTTT"), *b"AAAAAAAA");
        assert_eq!(reverse_complement(b"GGGGGGGG"), *b"CCCCCCCC");
    }

    #[test]
    fn test_double_reverse_complement_is_identity() {
        let original = b"ATGCGATC";
        let rc = reverse_complement(original);
        let rc_rc = reverse_complement(&rc);
        assert_eq!(&rc_rc, original);
    }
}


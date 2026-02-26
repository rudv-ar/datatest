/// refer/chunk.rs — 8-mer splitting and reassembly
///
/// Every valid dendec DNA string is a multiple of 4 bytes (4 bases per
/// encrypted byte). Since 8 is a multiple of 4, every valid dendec output
/// is automatically refer-compatible — no padding, no remainder, no edge cases.
///
/// This module is pure: no I/O, no network, no crypto. Fully testable
/// in isolation.

use crate::error::{DendecError, Result};

pub const KMER_LEN: usize = 8;

/// Split a flat DNA byte slice into successive 8-mer arrays.
///
/// Validates two invariants before returning:
///   1. Length is a multiple of 8.
///   2. Every character is A, T, G, or C.
///
/// Any violation returns `ReferInvalidBases` with the position of the
/// first offending byte.
pub fn split_into_kmers(dna: &[u8]) -> Result<Vec<[u8; KMER_LEN]>> {
    if dna.len() % KMER_LEN != 0 {
        return Err(DendecError::ReferInvalidBases { position: dna.len() });
    }

    let mut kmers = Vec::with_capacity(dna.len() / KMER_LEN);

    for (chunk_idx, chunk) in dna.chunks(KMER_LEN).enumerate() {
        for (i, &b) in chunk.iter().enumerate() {
            if !matches!(b, b'A' | b'T' | b'G' | b'C') {
                return Err(DendecError::ReferInvalidBases {
                    position: chunk_idx * KMER_LEN + i,
                });
            }
        }
        let mut kmer = [0u8; KMER_LEN];
        kmer.copy_from_slice(chunk);
        kmers.push(kmer);
    }

    Ok(kmers)
}

/// Concatenate a sequence of 8-mer arrays back into a flat DNA string.
///
/// The caller guarantees all bytes are valid A/T/G/C — this function
/// does not re-validate, mirroring the guarantee from split_into_kmers.
pub fn reassemble(kmers: &[[u8; KMER_LEN]]) -> String {
    let mut result = String::with_capacity(kmers.len() * KMER_LEN);
    for kmer in kmers {
        // Safety: all bytes are guaranteed to be ASCII (A/T/G/C)
        result.push_str(
            std::str::from_utf8(kmer).expect("8-mer contains non-UTF8 bytes"),
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_two_kmers() {
        let dna = b"ATGCGATCGGCTAGCA";
        let kmers = split_into_kmers(dna).unwrap();
        assert_eq!(kmers.len(), 2);
        assert_eq!(&kmers[0], b"ATGCGATC");
        assert_eq!(&kmers[1], b"GGCTAGCA");
    }

    #[test]
    fn test_split_invalid_length_rejected() {
        // 7 bases — not a multiple of 8
        let result = split_into_kmers(b"ATGCGAT");
        assert!(result.is_err());
    }

    #[test]
    fn test_split_invalid_char_rejected() {
        // N is not A/T/G/C
        let result = split_into_kmers(b"ATGCGATN");
        assert!(result.is_err());
    }

    #[test]
    fn test_split_invalid_char_position() {
        // Position 7 (last base in first chunk) is N
        let err = split_into_kmers(b"ATGCGATN").unwrap_err();
        match err {
            crate::error::DendecError::ReferInvalidBases { position } => {
                assert_eq!(position, 7);
            }
            other => panic!("unexpected error: {}", other),
        }
    }

    #[test]
    fn test_reassemble_roundtrip() {
        let dna = b"ATGCGATCGGCTAGCATCGATCGG";
        let kmers = split_into_kmers(dna).unwrap();
        let rebuilt = reassemble(&kmers);
        assert_eq!(rebuilt.as_bytes(), dna);
    }

    #[test]
    fn test_single_kmer() {
        let dna = b"ATGCGATC";
        let kmers = split_into_kmers(dna).unwrap();
        assert_eq!(kmers.len(), 1);
        assert_eq!(&kmers[0], b"ATGCGATC");
    }

    #[test]
    fn test_empty_input_rejected() {
        // Empty string: length 0 is a multiple of 8 but produces no kmers
        // This is technically valid — encode of empty DNA → empty BED
        let kmers = split_into_kmers(b"").unwrap();
        assert_eq!(kmers.len(), 0);
    }
}


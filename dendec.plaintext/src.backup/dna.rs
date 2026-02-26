/// dna.rs — Binary ↔ DNA base conversion
///
/// DNA mapping: each DNA base represents exactly 2 bits.
/// The mapping table itself is key-derived (see crypto::derive_dna_mapping).
///
/// Encoding:
///   bytes → iterate bits MSB-first → pair into 2-bit diits → lookup base
///
/// Decoding:
///   bases → reverse lookup → 2-bit digits → reassemble bytes
use crate::error::{DendecError, Result};

/// Convert a byte slice to a DNA string using the provided base mapping.
///
/// `mapping[0b00]` = base for 00, `mapping[0b01]` = base for 01, etc.
/// Bits are processed MSB-first within each byte, which ensures a
/// deterministic, byte-aligned encoding.
pub fn bytes_to_dna(bytes: &[u8], mapping: &[u8; 4]) -> String {
    let mut dna = String::with_capacity(bytes.len() * 4);
    for &byte in bytes {
        // Extract 4 pairs of 2 bits from MSB to LSB
        for shift in [6u8, 4, 2, 0] {
            let two_bits = ((byte >> shift) & 0b11) as usize;
            dna.push(mapping[two_bits] as char);
        }
    }
    dna
}

/// Convert a DNA string back to bytes using the provided base mapping.
///
/// Validates each character and reassembles 8-bit bytes from 4 bases each.
pub fn dna_to_bytes(dna: &str, mapping: &[u8; 4]) -> Result<Vec<u8>> {
    // Build reverse mapping: base char → 2-bit value
    let mut reverse = [None::<u8>; 128];
    for (i, &base) in mapping.iter().enumerate() {
        if (base as usize) < 128 {
            reverse[base as usize] = Some(i as u8);
        }
    }

    let chars: Vec<char> = dna.chars().collect();

    if chars.len() % 4 != 0 {
        return Err(DendecError::InvalidDnaLength(chars.len()));
    }

    let mut bytes = Vec::with_capacity(chars.len() / 4);

    for (chunk_idx, chunk) in chars.chunks(4).enumerate() {
        let mut byte = 0u8;
        for (bit_idx, &ch) in chunk.iter().enumerate() {
            let ascii = ch as usize;
            let two_bits = if ascii < 128 {
                reverse[ascii]
            } else {
                None
            };
            match two_bits {
                Some(v) => {
                    // MSB-first: first pair goes to bits 7-6
                    byte |= v << (6 - bit_idx * 2);
                }
                None => {
                    return Err(DendecError::InvalidDnaChar(ch, chunk_idx * 4 + bit_idx));
                }
            }
        }
        bytes.push(byte);
    }

    Ok(bytes)
}

/// Format a DNA string into groups of `n` bases separated by spaces.
/// Example: group_dna("ATGCATGC", 4) → "ATGC ATGC"
pub fn group_dna(dna: &str, n: usize) -> String {
    if n == 0 {
        return dna.to_string();
    }
    dna.as_bytes()
        .chunks(n)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT_MAPPING: [u8; 4] = [b'A', b'T', b'G', b'C'];

    #[test]
    fn test_roundtrip_ascii() {
        let original = b"Hello, World!";
        let dna = bytes_to_dna(original, &DEFAULT_MAPPING);
        let decoded = dna_to_bytes(&dna, &DEFAULT_MAPPING).unwrap();
        assert_eq!(original.as_ref(), decoded.as_slice());
    }

    #[test]
    fn test_roundtrip_emoji() {
        let original = "🧬🔐✨".as_bytes();
        let dna = bytes_to_dna(original, &DEFAULT_MAPPING);
        let decoded = dna_to_bytes(&dna, &DEFAULT_MAPPING).unwrap();
        assert_eq!(original, decoded.as_slice());
    }

    #[test]
    fn test_only_valid_bases() {
        let dna = bytes_to_dna(b"test data", &DEFAULT_MAPPING);
        for ch in dna.chars() {
            assert!(
                matches!(ch, 'A' | 'T' | 'G' | 'C'),
                "Unexpected char: {ch}"
            );
        }
    }

    #[test]
    fn test_invalid_char_rejected() {
        let result = dna_to_bytes("ATGX", &DEFAULT_MAPPING);
        assert!(result.is_err());
    }

    #[test]
    fn test_odd_length_rejected() {
        let result = dna_to_bytes("ATG", &DEFAULT_MAPPING);
        assert!(result.is_err());
    }

    #[test]
    fn test_custom_mapping_roundtrip() {
        let mapping: [u8; 4] = [b'G', b'A', b'C', b'T'];
        let original = b"custom mapping test";
        let dna = bytes_to_dna(original, &mapping);
        let decoded = dna_to_bytes(&dna, &mapping).unwrap();
        assert_eq!(original.as_ref(), decoded.as_slice());
    }

    #[test]
    fn test_group_dna() {
        let grouped = group_dna("ATGCATGC", 4);
        assert_eq!(grouped, "ATGC ATGC");
    }

    #[test]
    fn test_zero_byte() {
        let original = &[0u8];
        let dna = bytes_to_dna(original, &DEFAULT_MAPPING);
        assert_eq!(dna, "AAAA");
        let decoded = dna_to_bytes(&dna, &DEFAULT_MAPPING).unwrap();
        assert_eq!(original.as_ref(), decoded.as_slice());
    }

    #[test]
    fn test_max_byte() {
        let original = &[0xFFu8];
        let dna = bytes_to_dna(original, &DEFAULT_MAPPING);
        assert_eq!(dna, "CCCC");
    }
}


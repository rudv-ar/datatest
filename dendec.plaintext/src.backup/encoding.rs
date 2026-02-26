/// encoding.rs — Header format and full encode/decode pipelines
///
/// HEADER FORMAT (binary, stored inside the DNA payload)
/// ──────────────────────────────────────────────────────
///
///  Offset  Len   Field
///  ──────  ───   ─────────────────────────────────────────────────
///  0       4     Magic bytes: 0x44 0x4E 0x44 0x43  ("DNDC")
///  4       1     Version: 0x01
///  5       16    Argon2 salt  (random, 128 bits)
///  21      12    ChaCha20 nonce  (random, 96 bits)
///  33      8     Payload length (u64 little-endian, encrypted ciphertext size)
///  ──────  ───   ─────────────────────────────────────────────────
///  41      N     Ciphertext (plaintext bytes encrypted with Poly1305 tag)
///
/// Total header: 41 bytes → 164 DNA bases
use crate::crypto::{
    decrypt, derive_dna_mapping, derive_keys, derive_keys_with_salt, encrypt, NONCE_LEN, SALT_LEN,
};
use crate::dna::{bytes_to_dna, dna_to_bytes, group_dna};
use crate::error::{DendecError, Result};

const MAGIC: [u8; 4] = [0x44, 0x4E, 0x44, 0x43];
const VERSION: u8 = 0x01;

struct Header {
    salt: [u8; SALT_LEN],
    nonce: [u8; NONCE_LEN],
    payload_len: u64,
}

fn build_packet(header: &Header, ciphertext: &[u8]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(41 + ciphertext.len());
    packet.extend_from_slice(&MAGIC);
    packet.push(VERSION);
    packet.extend_from_slice(&header.salt);
    packet.extend_from_slice(&header.nonce);
    packet.extend_from_slice(&header.payload_len.to_le_bytes());
    packet.extend_from_slice(ciphertext);
    packet
}

fn parse_packet(packet: &[u8]) -> Result<(Header, &[u8])> {
    if packet.len() < 42 {
        return Err(DendecError::BadMagic);
    }
    if &packet[0..4] != MAGIC {
        return Err(DendecError::BadMagic);
    }

    let version = packet[4];
    if version != VERSION {
        return Err(DendecError::UnsupportedVersion {
            expected: VERSION,
            got: version,
        });
    }

    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&packet[5..21]);

    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&packet[21..33]);

    let mut len_bytes = [0u8; 8];
    len_bytes.copy_from_slice(&packet[33..41]);
    let payload_len = u64::from_le_bytes(len_bytes);

    let ciphertext = &packet[41..];
    if ciphertext.len() != payload_len as usize {
        return Err(DendecError::LengthMismatch {
            header: payload_len as usize,
            actual: ciphertext.len(),
        });
    }

    Ok((Header { salt, nonce, payload_len }, ciphertext))
}

/// Core encode — operates on raw bytes.
///
/// This is the canonical implementation. Both `encode` (text path)
/// and file-mode encoding call this. Accepts any byte slice, so binary
/// files, UTF-8 text, and partial content are all handled identically.
pub fn encode_raw(plaintext: &[u8], password: &str, group: Option<usize>) -> Result<String> {
    let keys = derive_keys(password)?;
    let (nonce, ciphertext) = encrypt(&keys.cipher_key, plaintext)?;

    let header = Header {
        salt: keys.salt,
        nonce,
        payload_len: ciphertext.len() as u64,
    };
    let packet = build_packet(&header, &ciphertext);
    let mapping = derive_dna_mapping(keys.mapping_seed);
    let dna = bytes_to_dna(&packet, &mapping);

    if let Some(n) = group {
        Ok(group_dna(&dna, n))
    } else {
        Ok(dna)
    }
}

/// Encode Unicode text into an encrypted DNA sequence.
///
/// Convenience wrapper around encode_raw for inline text input.
/// This part of code will be used by wrap subcommand, so it is not a dead code 
#[allow(dead_code)]
pub fn encode(text: &str, password: &str, group: Option<usize>) -> Result<String> {
    encode_raw(text.as_bytes(), password, group)
}

/// Core decode — returns raw bytes.
///
/// This is the canonical implementation. Both `decode` (text path)
/// and file-mode decoding call this. The caller decides whether to
/// interpret the result as UTF-8 or write it verbatim to a file.
pub fn decode_raw(dna: &str, password: &str) -> Result<Vec<u8>> {
    let dna_clean: String = dna.chars().filter(|c| !c.is_whitespace()).collect();

    let header_dna_len = 41 * 4;
    if dna_clean.len() < header_dna_len {
        return Err(DendecError::BadMagic);
    }

    let header_dna = &dna_clean[..header_dna_len];
    let bases = [b'A', b'T', b'G', b'C'];
    let permutations = all_permutations(&bases);

    let mut found_mapping: Option<[u8; 4]> = None;

    'outer: for perm in &permutations {
        if let Ok(header_bytes) = dna_to_bytes(header_dna, perm) {
            if &header_bytes[0..4] == MAGIC && header_bytes[4] == VERSION {
                let mut salt = [0u8; SALT_LEN];
                salt.copy_from_slice(&header_bytes[5..21]);
                let keys = derive_keys_with_salt(password, &salt)?;
                let expected_mapping = derive_dna_mapping(keys.mapping_seed);
                if expected_mapping == *perm {
                    found_mapping = Some(*perm);
                    break 'outer;
                }
            }
        }
    }

    let mapping = found_mapping.ok_or(DendecError::BadMagic)?;

    let packet = dna_to_bytes(&dna_clean, &mapping)?;
    let (header, ciphertext) = parse_packet(&packet)?;
    let keys = derive_keys_with_salt(password, &header.salt)?;
    let plaintext = decrypt(&keys.cipher_key, &header.nonce, ciphertext)?;

    Ok(plaintext)
}

/// Decode an encrypted DNA sequence back to Unicode text.
///
/// Convenience wrapper around decode_raw for inline text output.
/// Fails with a clear error if the decrypted bytes are not valid UTF-8.
/// To be used by the wrap subcommand again, not a dead code 
#[allow(dead_code)]
pub fn decode(dna: &str, password: &str) -> Result<String> {
    let bytes = decode_raw(dna, password)?;
    String::from_utf8(bytes).map_err(DendecError::Utf8)
}

fn all_permutations(arr: &[u8; 4]) -> Vec<[u8; 4]> {
    let mut result = Vec::with_capacity(24);
    let mut a = *arr;
    let mut c = [0usize; 4];
    result.push(a);
    let mut i = 0;
    while i < 4 {
        if c[i] < i {
            if i % 2 == 0 {
                a.swap(0, i);
            } else {
                a.swap(c[i], i);
            }
            result.push(a);
            c[i] += 1;
            i = 0;
        } else {
            c[i] = 0;
            i += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_ascii() {
        let text = "Hello, World!";
        let password = "correct-horse-battery-staple";
        let dna = encode(text, password, None).unwrap();
        let decoded = decode(&dna, password).unwrap();
        assert_eq!(text, decoded);
    }

    #[test]
    fn test_encode_decode_emoji() {
        let text = "DNA 🧬 is cool! ✨ テスト";
        let password = "p@ssw0rd!";
        let dna = encode(text, password, None).unwrap();
        let decoded = decode(&dna, password).unwrap();
        assert_eq!(text, decoded);
    }

    #[test]
    fn test_encode_raw_roundtrip() {
        // Raw bytes roundtrip — trailing newline preserved
        let bytes = b"exact bytes\nwith newline\n";
        let password = "rawtest";
        let dna = encode_raw(bytes, password, None).unwrap();
        let decoded = decode_raw(&dna, password).unwrap();
        assert_eq!(bytes.as_ref(), decoded.as_slice());
    }

    #[test]
    fn test_raw_binary_roundtrip() {
        // Non-UTF-8 binary bytes must roundtrip cleanly
        let bytes: Vec<u8> = (0u8..=255u8).collect();
        let password = "binarytest";
        let dna = encode_raw(&bytes, password, None).unwrap();
        let decoded = decode_raw(&dna, password).unwrap();
        assert_eq!(bytes, decoded);
    }

    #[test]
    fn test_wrong_password_fails() {
        let text = "Secret message";
        let dna = encode(text, "correct-password", None).unwrap();
        let result = decode(&dna, "wrong-password");
        assert!(result.is_err());
    }

    #[test]
    fn test_grouped_output_decodes() {
        let text = "grouped output test";
        let password = "testpass";
        let dna = encode(text, password, Some(10)).unwrap();
        let decoded = decode(&dna, password).unwrap();
        assert_eq!(text, decoded);
    }

    #[test]
    fn test_all_permutations_count() {
        let arr = [b'A', b'T', b'G', b'C'];
        let perms = all_permutations(&arr);
        assert_eq!(perms.len(), 24);
        let mut sorted = perms.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 24);
    }

    #[test]
    fn test_different_passwords_produce_different_dna() {
        let text = "Same text";
        let dna1 = encode(text, "password1", None).unwrap();
        let dna2 = encode(text, "password2", None).unwrap();
        assert_ne!(dna1, dna2);
    }

    #[test]
    fn test_same_password_different_each_time() {
        let text = "Same text";
        let dna1 = encode(text, "mypassword", None).unwrap();
        let dna2 = encode(text, "mypassword", None).unwrap();
        assert_ne!(dna1, dna2);
    }

    #[test]
    fn test_corrupted_dna_fails() {
        let text = "Integrity check";
        let mut dna = encode(text, "mypassword", None).unwrap();
        let flip_pos = dna.len() - 10;
        let bytes = unsafe { dna.as_bytes_mut() };
        bytes[flip_pos] = match bytes[flip_pos] {
            b'A' => b'T',
            b'T' => b'G',
            b'G' => b'C',
            b'C' => b'A',
            _ => b'A',
        };
        let result = decode(&dna, "mypassword");
        assert!(result.is_err());
    }

    #[test]
    fn test_unicode_newlines_tabs() {
        let text = "Line one\n\tTabbed line\nLine three 日本語";
        let password = "unicode-test";
        let dna = encode(text, password, None).unwrap();
        let decoded = decode(&dna, password).unwrap();
        assert_eq!(text, decoded);
    }
}

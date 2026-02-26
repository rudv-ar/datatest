/// crypto.rs — Key derivation and authenticated encryption
///
/// WHY Argon2id?
///   Argon2id is the winner of the Password Hashing Competition (2015).
///   It is memory-hard (resists GPU/ASIC attacks) and combines the
///   data-dependent Argon2d (GPU resistance) with data-independent
///   Argon2i (side-channel resistance). It is the current OWASP/NIST
///   recommended password KDF.
///
/// WHY a random salt?
///   A fresh random salt per encode ensures that the same password
///   produces a different key each time. This prevents pre-computation
///   (rainbow table) attacks and ensures that two users with the same
///   password cannot correlate their outputs.
///
/// WHY ChaCha20-Poly1305?
///   ChaCha20-Poly1305 is an AEAD (Authenticated Encryption with
///   Associated Data) cipher. It provides:
///     - Confidentiality: ChaCha20 stream cipher
///     - Integrity + Authenticity: Poly1305 MAC
///   It is fast on platforms without AES hardware, constant-time by
///   design, and mandated in TLS 1.3. The 256-bit key offers a
///   128-bit security margin against generic attacks.
///
/// WHY a random nonce?
///   ChaCha20-Poly1305 is catastrophically broken if a nonce is ever
///   reused with the same key. A 96-bit random nonce has a collision
///   probability of ~2^-33 after 2^32 messages — safe for our use
///   case. The nonce is stored in the header so decode can recover it.
use crate::error::{DendecError, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use rand::{RngCore, SeedableRng};
use rand::rngs::StdRng;

/// Size constants
pub const SALT_LEN: usize = 16; // 128-bit salt for Argon2
pub const NONCE_LEN: usize = 12; // 96-bit nonce for ChaCha20-Poly1305
pub const KEY_LEN: usize = 32;  // 256-bit ChaCha20 key
pub const MAPPING_SEED_LEN: usize = 8; // 64-bit seed for DNA mapping RNG

/// Argon2id parameters (OWASP "interactive" tier, adjustable)
const ARGON2_M_COST: u32 = 65536; // 64 MiB memory
const ARGON2_T_COST: u32 = 3;     // 3 iterations
const ARGON2_P_COST: u32 = 1;     // 1 thread (CLI context)

/// Output of key derivation — everything needed for one session
pub struct DerivedKeys {
    /// 256-bit key for ChaCha20-Poly1305
    pub cipher_key: [u8; KEY_LEN],
    /// 64-bit seed used to derive the DNA base mapping order
    pub mapping_seed: u64,
    /// The salt used (must be stored in header for decode)
    pub salt: [u8; SALT_LEN],
}

/// Derive all session keys from a password.
///
/// Generates a fresh random salt, then runs Argon2id to produce
/// 40 bytes of key material: 32 bytes for the cipher key and
/// 8 bytes for the DNA mapping seed.
pub fn derive_keys(password: &str) -> Result<DerivedKeys> {
    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    derive_keys_with_salt(password, &salt)
}

/// Derive keys from a password and an existing salt (used during decode).
pub fn derive_keys_with_salt(password: &str, salt: &[u8; SALT_LEN]) -> Result<DerivedKeys> {
    let params = Params::new(ARGON2_M_COST, ARGON2_T_COST, ARGON2_P_COST, Some(KEY_LEN + MAPPING_SEED_LEN))
        .map_err(|e| DendecError::KeyDerivation(e.to_string()))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut output = [0u8; KEY_LEN + MAPPING_SEED_LEN];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut output)
        .map_err(|e| DendecError::KeyDerivation(e.to_string()))?;

    let mut cipher_key = [0u8; KEY_LEN];
    cipher_key.copy_from_slice(&output[..KEY_LEN]);

    // Extract 8 bytes as a little-endian u64 seed for the DNA RNG
    let mut seed_bytes = [0u8; MAPPING_SEED_LEN];
    seed_bytes.copy_from_slice(&output[KEY_LEN..]);
    let mapping_seed = u64::from_le_bytes(seed_bytes);

    Ok(DerivedKeys {
        cipher_key,
        mapping_seed,
        salt: *salt,
    })
}

/// Encrypt plaintext bytes with ChaCha20-Poly1305.
///
/// Returns (nonce, ciphertext). The nonce is randomly generated and
/// must be stored in the header for decryption.
pub fn encrypt(key: &[u8; KEY_LEN], plaintext: &[u8]) -> Result<([u8; NONCE_LEN], Vec<u8>)> {
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| DendecError::DecryptionFailed)?;

    Ok((nonce_bytes, ciphertext))
}

/// Decrypt ciphertext bytes with ChaCha20-Poly1305.
///
/// The Poly1305 MAC is verified automatically — if the password is
/// wrong or the data is corrupted, decryption returns an error.
pub fn decrypt(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN], ciphertext: &[u8]) -> Result<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = Nonce::from_slice(nonce);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| DendecError::DecryptionFailed)
}

/// Derive the DNA base permutation from the mapping seed.
///
/// WHY a permuted mapping?
///   Using a static 00→A 01→T 10→G 11→C mapping would make all
///   dendec outputs structurally equivalent — an attacker could
///   skip the KDF and directly brute-force the 2-bit→base mapping.
///   By deriving the mapping from the password, the DNA alphabet
///   itself becomes key-dependent, adding another layer of obscurity
///   (defence-in-depth on top of the AEAD guarantee).
///
/// The seed is used to initialise a ChaCha-based CSPRNG (StdRng via
/// seeded_from_u64). The four bases [A, T, G, C] are shuffled with
/// a Fisher-Yates shuffle. Same password + same salt → same shuffle.
pub fn derive_dna_mapping(mapping_seed: u64) -> [u8; 4] {
    let mut rng = StdRng::seed_from_u64(mapping_seed);
    let mut bases: [u8; 4] = [b'A', b'T', b'G', b'C'];
    // Fisher-Yates shuffle
    for i in (1..4).rev() {
        let j = (rng.next_u64() as usize) % (i + 1);
        bases.swap(i, j);
    }
    bases
}


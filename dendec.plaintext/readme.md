![dendec-logo](./assets/dendec.jpg)


**DNA Encode and Decode** — Password-based encrypted Unicode to DNA encoding, Shortly known as dendec is a cli tool for steganographic obfuscation alongside with encryption. This will give you a DNA sequence which can be synthesized via `dendec refer` (comming soon) to point out to real genomic databases. Data is already present, you would just need coordinates and password.

The world's first general-purpose DNA-native encrypted data format. Your data becomes a sequence of nucleotide bases. Locked behind production-grade cryptography. Indistinguishable from genomic output. Decodable only with `dendec` and the correct password.

```
$ dendec encode "Hello, World!"
Enter password:
Confirm password:
Encoding… (Argon2id key derivation may take a moment)
GCATCGATCGGCTAGCATCGATCGGCTAGCATCGATCGGCTAGCAT...
```

> [!IMPORTANT]
> dendec is not a toy. The cryptographic primitives underneath are production-grade. Treat encoded output and passwords with the same seriousness you would any encrypted data.


## &#xe897; What is dendec

Most encryption tools announce themselves. PGP output looks like PGP. AES-encrypted blobs look encrypted. Even steganography tools hide data in known ways with known detectors.

dendec is different in a fundamental sense.

It encodes your data using the vocabulary of molecular biology — A, T, G, C — the same four bases that constitute every genome ever sequenced. The output is visually and structurally indistinguishable from a real DNA sequencing result. No armored headers. No base64 padding. Nothing that flags as ciphertext to any scanner trained on conventional encrypted formats.

Under the surface it is cryptographically serious:

- Argon2id key derivation — memory-hard, GPU and ASIC resistant, OWASP and NIST recommended
- ChaCha20-Poly1305 authenticated encryption — simultaneous confidentiality and integrity
- Key-derived DNA alphabet — the base mapping itself is derived from the password
- Random salt and nonce per encode — identical inputs never produce identical output
- Self-contained output — salt, nonce, version, and ciphertext all embedded in the sequence itself


## &#xe8b8; Installation

### From source

```bash
git clone https://github.com/dendec/dendec
cd dendec
cargo build --release
```

The compiled binary will be at `./target/release/dendec`.

### Requirements

- Rust 1.75 or later
- Cargo


## &#xe869; Usage

### Encode text

```bash
dendec encode "Your secret message"
```

All Unicode is supported — emoji, CJK characters, Arabic, newlines, tabs, every valid UTF-8 sequence.

### Encode with grouped output

```bash
dendec encode "Your secret message" --group 10
```

Output:
```
ATGCATGCAT GCATGCATGC TAGCTAGCAT...
```

Grouping is cosmetic only. The decoder strips whitespace automatically.

### Decode

```bash
dendec decode "ATGCTAGCAT..."
```

### Encode a file via shell

```bash
dendec encode "$(cat file.txt)" > file.txt.dna
```

> [!WARNING]
> Shell command substitution via `$()` strips trailing newlines from file content. Use the `--file` flag (coming soon) for binary-safe file encoding that preserves exact byte content.

### Decode a file

```bash
dendec decode "$(cat file.txt.dna)"
```


## &#xe90d; How It Works

### Encode pipeline

```
Unicode text
    │
    ▼
UTF-8 bytes
    │
    ▼
Argon2id(password, random_salt)
    ├──► cipher_key      [256 bits — ChaCha20 key]
    └──► mapping_seed    [64 bits  — DNA shuffle seed]
    │
    ▼
Fisher-Yates shuffle([A,T,G,C], mapping_seed) ──► DNA mapping table
    │
    ▼
ChaCha20-Poly1305(plaintext, cipher_key, random_nonce) ──► ciphertext
    │
    ▼
Binary packet: [DNDC][v1][salt 16B][nonce 12B][payload_len 8B][ciphertext]
    │
    ▼
2 bits per base: 00→X  01→X  10→X  11→X  (X determined by mapping table)
    │
    ▼
Output: GCATCGATCGGCTAGC...
```

### Binary header format

The header is embedded directly into the DNA sequence as the first 41 bytes, which corresponds to the first 164 bases of any dendec output.

```
Offset   Length   Field
───────  ──────   ──────────────────────────────────────────
0        4        Magic bytes  0x44 0x4E 0x44 0x43  ("DNDC")
4        1        Version      0x01
5        16       Argon2id salt         (random, 128 bits)
21       12       ChaCha20-Poly1305 nonce  (random, 96 bits)
33       8        Payload length        (u64 little-endian)
41       N        Ciphertext            (payload + 16 byte MAC)
```

Everything required for decryption lives inside the DNA string itself. No sidecar files. No external configuration. The sequence is the complete artifact.

### Decode pipeline

```
DNA string
    │
    ▼
Strip whitespace and grouping separators
    │
    ▼
Try all 24 permutations of [A,T,G,C] against first 164 bases
    │   For each permutation:
    │     → decode to bytes
    │     → check for magic bytes "DNDC"
    │     → extract salt
    │     → run Argon2id(password, salt) → derive expected mapping
    │     → confirm derived mapping matches current permutation
    ▼
Confirmed mapping recovered
    │
    ▼
Decode full DNA string → binary packet
    │
    ▼
Parse header → extract salt, nonce, payload_len
    │
    ▼
Argon2id(password, salt) ──► cipher_key
    │
    ▼
ChaCha20-Poly1305 decrypt and verify MAC
    ├── correct password  → plaintext bytes returned
    ├── wrong password    → MAC mismatch → DecryptionFailed error
    └── corrupted data    → MAC mismatch → DecryptionFailed error
    │
    ▼
String::from_utf8 ──► original text
```

> [!NOTE]
> The bootstrap loop tries at most 24 permutations. For each candidate it runs Argon2id once to verify the mapping. In practice the correct permutation is found on the first or second attempt. The upper bound is 24 Argon2id invocations which remains under 30 seconds on typical hardware.


## &#xe32a; Security

### Cryptographic primitives

| Primitive | Role | Rationale |
|---|---|---|
| Argon2id | Password to key | Winner of Password Hashing Competition 2015. Memory-hard. Combines data-dependent and data-independent hardness. Current OWASP and NIST recommendation. |
| ChaCha20-Poly1305 | Encryption and authentication | AEAD construction. Constant-time by design. Mandated in TLS 1.3. Poly1305 MAC ensures any tampering is detected before plaintext is returned. |
| StdRng seeded from key material | DNA mapping shuffle | ChaCha-based CSPRNG. Seeded from Argon2id output, not the password directly. Deterministic given the same key. |
| rand::thread_rng | Salt and nonce generation | OS-seeded CSPRNG. Fresh 128-bit salt and 96-bit nonce per encode operation. |

### Argon2id parameters

| Parameter | Value | Effect |
|---|---|---|
| Memory cost | 65536 KiB (64 MiB) | RAM required per guess |
| Time cost | 3 iterations | CPU cost multiplier |
| Parallelism | 1 | Single-threaded |

At these parameters each password guess costs approximately 64 MiB of RAM and one second of wall time. An attacker with substantial hardware resources still faces centuries of work against a strong passphrase.

### Threat model

| Attack vector | Mitigation |
|---|---|
| Wrong password | Poly1305 MAC fails before any plaintext is returned |
| Corrupted or tampered DNA | MAC fails, clean error, no partial output |
| Rainbow table precomputation | Blocked by 128-bit random salt. Same password never produces the same key. |
| Nonce reuse | Impossible. Fresh random nonce generated per encode. |
| Mapping brute-force (24 permutations) | Each permutation still hits ChaCha20-Poly1305. No shortcut past the KDF. |
| Visual identification of ciphertext | Output is valid nucleotide notation. Unrecognisable as encrypted data to conventional scanners. |

> [!CAUTION]
> dendec does not currently zeroize keys and passwords from process memory after use. On shared or compromised systems a memory dump could expose key material. Zeroization via the `zeroize` crate is on the roadmap.

> [!WARNING]
> dendec cannot protect against weak passwords. The strength of Argon2id is irrelevant if the passphrase is guessable. Use a long random passphrase.


## &#xe86f; Project Structure

```
dendec/
├── Cargo.toml
├── LICENSE
├── README.md
└── src/
    ├── main.rs        Entry point. CLI dispatch and password prompts. No crypto logic.
    ├── cli.rs         clap v4 derive API. Subcommand and flag definitions.
    ├── crypto.rs      Argon2id KDF. ChaCha20-Poly1305 encrypt and decrypt. Mapping derivation.
    ├── encoding.rs    Binary header format. Full encode and decode pipeline. Bootstrap logic.
    ├── dna.rs         Bit-level bytes to DNA and DNA to bytes conversion. Grouping utility.
    └── error.rs       Custom error enum via thiserror. No panics in production paths.
```


## &#xe5c3; Dependencies

| Crate | Version | Purpose |
|---|---|---|
| `clap` | 4 | CLI argument parsing via derive API |
| `rpassword` | 7 | Hidden password prompt, no terminal echo |
| `argon2` | 0.5 | Argon2id key derivation |
| `rand` | 0.8 | Cryptographically secure salt and nonce generation |
| `chacha20poly1305` | 0.10 | ChaCha20-Poly1305 AEAD encryption |
| `thiserror` | 1 | Ergonomic custom error types |


## &#xe877; Tests

```bash
cargo test
```

18 tests across two modules:

```
dna::tests
  test_roundtrip_ascii
  test_roundtrip_emoji
  test_only_valid_bases
  test_invalid_char_rejected
  test_odd_length_rejected
  test_custom_mapping_roundtrip
  test_group_dna
  test_zero_byte
  test_max_byte

encoding::tests
  test_encode_decode_ascii
  test_encode_decode_emoji
  test_wrong_password_fails
  test_corrupted_dna_fails
  test_grouped_output_decodes
  test_different_passwords_produce_different_dna
  test_same_password_different_each_time
  test_unicode_newlines_tabs
  test_all_permutations_count
```

> [!NOTE]
> The test suite takes approximately 80 seconds to complete. This is expected and correct. Each encode and decode operation pays the full Argon2id cost. Reduced test times would indicate the KDF is not functioning as intended.


## &#xe0b7; The Bigger Vision

dendec is the core of a three-layer architecture.

### Layer 1 — dendec core — `stable`

Password-encrypted data serialised as A/T/G/C bases. Fully implemented, tested, and documented.

### Layer 2 — dendec wrap — `in development`

A protocol-agnostic fetch-and-transform layer. Wraps any command that retrieves data. Encodes all readable files to `.dna` format in place. Preserves directory structure exactly. The transformed repository can be pushed to GitHub, hosted anywhere, fetched by anyone — and appears to contain genomics data.

```bash
dendec wrap -e git clone https://github.com/user/repo
dendec wrap -e curl https://example.com/config.rs
dendec wrap -d git clone https://github.com/user/repo
dendec wrap -d curl https://example.com/config.rs.dna
```

The `.dna` extension is the complete protocol. Any observer sees file annotations that look like biology research artifacts.

### Layer 3 — dendec refer — `planned`

The most architecturally significant layer. Instead of transmitting DNA bases, transmit coordinates into public genome databases. The bases themselves are never present in the message. They exist latent inside the sequenced genomes of real organisms — distributed across decades of publicly funded biology research — and are reconstructable only by someone who knows the correct coordinates and holds the correct password.

```bash
dendec refer encode output.dna      # DNA bases to genomic coordinate references
dendec refer decode references.txt  # Fetch genome slices, reconstruct, decrypt
```

An intercepted `references.txt` is indistinguishable from a researcher's genomic annotation notes.

> [!NOTE]
> With `dendec refer` fully implemented, no encrypted data exists in transit. The transmitted artifact is a list of coordinates into public biological databases. The message is latent inside the history of life on Earth.


## &#xe14b; TODO — Upcoming Features

### Near term

- [ ] `--file` flag — binary-safe file encode and decode without shell substitution
- [ ] `--output` flag — write result directly to a file instead of stdout
- [ ] Trailing newline preservation in file mode
- [ ] Richer terminal output — input size, output length, base statistics, elapsed time
- [ ] `--quiet` flag — suppress all stderr output for scripting

### dendec wrap

- [ ] `dendec wrap -e <command>` — encode all readable files from any fetch command
- [ ] `dendec wrap -d <command>` — decode all `.dna` files from any fetch command
- [ ] Git repository support with full directory structure preservation
- [ ] curl and wget single-file support
- [ ] Binary file detection via byte sampling — non-text files skipped automatically
- [ ] `.git/` directory exclusion
- [ ] Progress indication for multi-file operations
- [ ] Summary report — file count, total size, elapsed time

### Compression

- [ ] `--compress` flag — zstd compression applied before encryption
- [ ] Compression flag stored in binary header
- [ ] Compression ratio reported on encode

### Security hardening

- [ ] Zeroize keys and password material from memory after use (`zeroize` crate)
- [ ] `--iterations` and `--memory` flags for Argon2id parameter tuning
- [ ] Full timing side-channel audit

### dendec refer

- [ ] Coordinate format specification and versioning
- [ ] NCBI RefSeq API integration with pinned assembly version
- [ ] Substring search against genome database
- [ ] Ambiguity resolution strategy for repeated substrings
- [ ] `dendec refer encode` — DNA string to genomic coordinate list
- [ ] `dendec refer decode` — coordinate list to DNA string to plaintext

### Testing and distribution

- [ ] CLI integration tests via `assert_cmd`
- [ ] Fuzz testing on the DNA parser
- [ ] Cross-platform CI — Linux, macOS, Windows
- [ ] Benchmark suite for Argon2id parameter selection
- [ ] Publish to crates.io
- [ ] Pre-built binaries via GitHub releases
- [ ] Homebrew formula
- [ ] Man page


## &#xe838; Contributing

dendec is in active early development. The core is stable and fully tested. `wrap` is the next milestone.

The binary header format is versioned. Any future version of dendec will maintain backward compatibility with sequences encoded by v1. If you are building tooling on top of dendec, the header layout and magic bytes are stable.

Issues and pull requests are welcome.


## &#xe873; License

MIT — see `LICENSE`.


## &#xe0c8; Philosophy

Make encrypted data indistinguishable from nature.

Not indistinguishable from random noise — indistinguishable from biology. The entire history of life on Earth is written in four characters. Every organism that has ever existed used this alphabet. Every genome database in the world speaks it natively. No security scanner in existence is trained to treat it as a threat vector.

dendec encodes your secrets into that language. An intercepted sequence looks like a fragment from a sequencing run, a slice from a database export, a researcher's working data. The content is invisible not because it is hidden but because it belongs.

At full completion — with `dendec refer` implemented — the data does not exist in transit at all. It is latent inside the sequenced genomes of real organisms, distributed across petabases of publicly available biological data, reconstructable only by someone who knows precisely where to look and holds the correct password.

The bases were always there. dendec found them.


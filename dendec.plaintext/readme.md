![dendec-logo](./assets/dendec.jpg)


**DNA Encode and Decode** — Password-based encrypted Unicode to DNA encoding. Shortly known as dendec, it is a CLI tool for steganographic obfuscation alongside production-grade cryptography. Output is a DNA sequence visually indistinguishable from real genomic data. Via `dendec refer` (coming soon), that sequence can be synthesised into coordinates pointing to real genomic databases — the data was always there, you just need the coordinates and a password.

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
git clone https://github.com/rudv-ar/dendec
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

### Encode a file — binary-safe

```bash
dendec encode --file src/main.rs --as main.rs.dna
```

Reads raw bytes directly from disk. Every byte — including trailing newlines and binary content — is preserved exactly. No shell substitution mangling.

### Decode a file — binary-safe

```bash
dendec decode --file main.rs.dna --as main.rs
```

Writes raw bytes directly to the output file. Byte-for-byte identical to the original.

### Verify a roundtrip

```bash
dendec encode --file src/main.rs --as main.rs.dna
dendec decode --file main.rs.dna --as restored.rs
diff src/main.rs restored.rs
# empty — perfect roundtrip
```

> [!NOTE]
> The `--as` flag writes output directly to a named file and prints a confirmation line to stderr. Without `--as`, output goes to stdout.


## &#xe91c; wrap — Protocol-Agnostic Batch Transform

`dendec wrap` intercepts the output of any shell command and applies a DNA transform to every appropriate file it produces. Directory structure is preserved exactly. Binary files are detected and skipped automatically. One password covers the entire operation — each file still gets its own random salt and nonce internally.

### Encode a local directory

```bash
dendec wrap -e ./myproject
```

Walks the entire directory tree, encodes every readable file to `.dna` in place, removes the originals.

```
  Scanning ./myproject...

Encoding 15 file(s)...

  Encoding ./myproject/src/main.rs...      ok  (1.8 KB → 7.2 KB)
  Encoding ./myproject/src/lib.rs...       ok  (0.9 KB → 3.6 KB)
  Encoding ./myproject/Cargo.toml...       ok  (312 B → 1.2 KB)
  Encoding ./myproject/README.md...        ok  (4.1 KB → 16.4 KB)
  Skipping ./myproject/assets/logo.png     (binary)

  15 files encoded  |  1 skipped  |  0 failed
```

### Decode a local directory

```bash
dendec wrap -d ./myproject
```

Walks the directory, finds every `.dna` file, decodes each one back to its original bytes, removes the `.dna` file. The directory is restored to its exact pre-encode state.

### Wrap a git clone — encode

```bash
dendec wrap -e git clone https://github.com/user/repo
```

Snapshots the working directory, runs the clone, diffs the filesystem to find exactly what was produced, then encodes every readable file in the cloned directory. Push the result anywhere — it looks like a genomics data repository.

### Wrap a git clone — decode

```bash
dendec wrap -d git clone https://github.com/user/repo
```

Clones a repository containing `.dna` files and decodes them all in place. The result is the original working source tree.

### Wrap curl — file output

```bash
dendec wrap -d curl -o config.toml.dna https://example.com/config.toml.dna
```

curl writes to disk, the snapshot diff detects it, dendec decodes it.

### Wrap curl — stdout capture

```bash
dendec wrap -d curl https://example.com/file.rs.dna
```

dendec captures stdout from curl and decodes it directly without touching the filesystem.

### What wrap skips automatically

Binary files are detected by content inspection. The first 512 bytes are sampled using the same heuristic git uses. The following are always skipped regardless:

- `.git/` directory
- `target/` directory
- `node_modules/`, `.svn/`, `.hg/`
- Known binary extensions: `png jpg jpeg gif bmp ico webp tiff pdf zip tar gz bz2 xz wasm exe dll so dylib mp3 mp4 wav ogg flac avi mkv mov db sqlite pyc class`
- Files containing null bytes
- Files where more than 10% of sampled bytes are non-printable


## &#xe91c; Live Example — rudv-ar/datatest

The repository at `https://github.com/rudv-ar/datatest` is a live demonstration of dendec wrap in action. It holds the full dendec source tree in both DNA-encoded and plaintext form side by side for direct comparison.

```
datatest/
├── dendec.dna/                  ← full dendec source, DNA-encoded (password: rust)
│   ├── Cargo.lock.dna
│   ├── Cargo.toml.dna
│   ├── LICENSE.dna
│   ├── readme.md.dna
│   ├── assets/
│   │   └── dendec.jpg           ← binary, skipped by wrap automatically
│   ├── src/
│   │   ├── main.rs.dna
│   │   ├── cli.rs.dna
│   │   ├── crypto.rs.dna
│   │   ├── dna.rs.dna
│   │   ├── encoding.rs.dna
│   │   ├── error.rs.dna
│   │   └── wrap/
│   │       ├── mod.rs.dna
│   │       ├── classify.rs.dna
│   │       ├── fetch.rs.dna
│   │       ├── snapshot.rs.dna
│   │       └── transform.rs.dna
│   └── src.backup/
│       └── *.dna
└── dendec.plaintext/            ← original source for verification
    ├── Cargo.lock
    ├── Cargo.toml
    └── src/
        └── ...
```

### Clone and decode

```bash
git clone https://github.com/rudv-ar/datatest
dendec wrap -d ./datatest/dendec.dna
# Enter password: rust
```

### Verify against plaintext

```bash
diff -r datatest/dendec.plaintext datatest/dendec.dna
# empty — byte-for-byte identical after decode
```

### Or clone and decode in a single command

```bash
dendec wrap -d git clone https://github.com/rudv-ar/datatest
# Enter password: rust
# All .dna files inside datatest/ are decoded in place
```


## &#xe90d; How It Works

### Encode pipeline

```
Input (text or raw bytes from file)
    │
    ▼
UTF-8 bytes or raw binary
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
Output: GCATCGATCGGCTAGC...  (to stdout or --as file)
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

Everything required for decryption lives inside the DNA string itself. No sidecar files. No external configuration. No key exchange. The sequence is the complete artifact.

### Decode pipeline

```
DNA string or .dna file
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
Raw bytes → file (--as) or UTF-8 text → stdout
```

> [!NOTE]
> The bootstrap loop tries at most 24 permutations. For each candidate it runs Argon2id once to verify the mapping. In practice the correct permutation is found on the first or second attempt. The upper bound is 24 Argon2id invocations, well under 30 seconds on typical hardware.


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
    ├── main.rs          Entry point. CLI dispatch and password prompts. No crypto logic.
    ├── cli.rs           clap v4 derive API. Subcommand and flag definitions.
    ├── crypto.rs        Argon2id KDF. ChaCha20-Poly1305 encrypt and decrypt. Mapping derivation.
    ├── encoding.rs      Binary header format. Full encode and decode pipeline. Bootstrap logic.
    ├── dna.rs           Bit-level bytes to DNA and DNA to bytes conversion. Grouping utility.
    ├── error.rs         Custom error enum via thiserror. No panics in production paths.
    └── wrap/
        ├── mod.rs       Orchestration. Local dir shortcut. Git clone narrowing. Stdout capture.
        ├── snapshot.rs  Filesystem snapshot and diff. Detects exactly what a command produced.
        ├── classify.rs  Binary detection. Skip rules. Extension logic.
        ├── transform.rs Batch encode/decode. Per-file progress. Summary report.
        └── fetch.rs     Subprocess execution. Disk vs stdout detection. Git clone parsing.
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
| `walkdir` | 2 | Recursive directory traversal for wrap |
| `tempfile` | 3 | Temporary directories in tests (dev only) |


## &#xe877; Tests

```bash
cargo test
```

33 tests across five modules:

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
  test_encode_raw_roundtrip
  test_raw_binary_roundtrip
  test_wrong_password_fails
  test_grouped_output_decodes
  test_different_passwords_produce_different_dna
  test_same_password_different_each_time
  test_corrupted_dna_fails
  test_unicode_newlines_tabs
  test_all_permutations_count

wrap::snapshot::tests
  test_new_file_detected
  test_unchanged_file_not_in_diff
  test_empty_dir_snapshot

wrap::classify::tests
  test_dna_file_skipped_in_encode
  test_non_dna_skipped_in_decode
  test_dna_file_decoded_in_decode
  test_git_dir_excluded
  test_binary_extension_skipped
  test_text_file_encoded
  test_null_byte_is_binary

wrap::transform::tests
  test_encode_decode_file_roundtrip
  test_strip_dna_extension
  test_human_size
```

> [!NOTE]
> The test suite takes approximately 80 seconds to complete. This is expected and correct. Each encode and decode operation in the encoding tests pays the full Argon2id cost. Reduced test times would indicate the KDF is not functioning as intended.


## &#xe0b7; The Bigger Vision

dendec is the core of a three-layer architecture.

### Layer 1 — dendec core — `stable`

Password-encrypted data serialised as A/T/G/C bases. Fully implemented, tested, and documented. Supports inline text, binary-safe file I/O via `--file` and `--as`, and grouped terminal output.

### Layer 2 — dendec wrap — `stable`

A protocol-agnostic fetch-and-transform layer. Wraps any shell command that produces files. Encodes all readable files to `.dna` format in place, preserving directory structure exactly. The transformed repository can be pushed to GitHub, hosted anywhere, fetched by anyone — and appears to contain genomics data.

```bash
dendec wrap -e ./myproject
dendec wrap -d ./myproject
dendec wrap -e git clone https://github.com/user/repo
dendec wrap -d git clone https://github.com/user/repo
dendec wrap -e curl -o config.toml https://example.com/config.toml
dendec wrap -d curl -o config.toml.dna https://example.com/config.toml.dna
```

The `.dna` extension is the complete protocol. Any observer sees file annotations that look like biology research artifacts.

### Layer 3 — dendec refer — `planned`

The most architecturally significant layer. Instead of transmitting DNA bases, transmit coordinates into public genome databases. The bases themselves are never present in the message. They exist latent inside the sequenced genomes of real organisms — distributed across decades of publicly funded biology research — and are reconstructable only by someone who knows the correct coordinates and holds the correct password.

```bash
dendec refer encode output.dna      # DNA bases → genomic coordinate references
dendec refer decode references.txt  # fetch genome slices → reconstruct → decrypt
```

An intercepted `references.txt` is indistinguishable from a researcher's genomic annotation notes.

> [!NOTE]
> With `dendec refer` fully implemented, no encrypted data exists in transit. The transmitted artifact is a list of coordinates into public biological databases. The message is latent inside the history of life on Earth.


## &#xe14b; TODO — Upcoming Features

### Near term

- [x] `--file` flag — binary-safe file encode and decode
- [x] `--as` flag — write output directly to a named file
- [x] Exact byte preservation including trailing newlines and binary content
- [ ] `--quiet` flag — suppress all stderr output for scripting
- [ ] Richer terminal output — input size, output length, base count, elapsed time

### dendec wrap

- [x] Local directory transform — `dendec wrap -e ./dir`
- [x] Git repository support — `dendec wrap -e git clone <url>`
- [x] curl and wget support — disk and stdout modes
- [x] Binary file detection via content sampling
- [x] `.git/`, `target/`, `node_modules/` exclusion
- [x] Per-file progress reporting with sizes
- [x] Summary report — transformed, skipped, failed
- [ ] `--quiet` flag for scripting and CI
- [ ] `--dry-run` — show what would be transformed without doing it
- [ ] Elapsed time in summary report

### Compression

- [ ] `--compress` flag — zstd compression applied before encryption
- [ ] Compression flag stored in binary header version field
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

dendec is in active early development. The core and wrap layers are stable and fully tested. `refer` is the next milestone.

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


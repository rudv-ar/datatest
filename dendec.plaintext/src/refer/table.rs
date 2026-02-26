/// refer/table.rs — Embedded lookup table with forward and reverse indices
///
/// The pre-built table (data/table.bin) is embedded at compile time via
/// include_bytes!. At runtime, load() parses the binary once and builds
/// two indices in memory:
///
///   forward:  8-mer base-4 index → Vec<Coord>   (encode path, O(1))
///   reverse:  CoordKey → u16 8-mer index        (decode path, O(1))
///
/// The fixed base-4 mapping used here (A=0, T=1, G=2, C=3) is completely
/// independent of the key-derived permuted mapping in dendec core. Refer
/// treats the ATGC string as opaque characters — it never interprets the
/// cryptographic meaning of the bases.
///
/// BINARY FORMAT (data/table.bin)
/// ─────────────────────────────────────────────────────────────────────
///  Offset  Len   Field
///  0       4     Magic: 0x44 0x52 0x46 0x54  ("DRFT")
///  4       1     Version: 0x01
///  5       2     Chromosome count (u16 LE)
///  7       var   Accession strings: [len: u8][utf8 bytes] × count
///  ?       var   65,536 8-mer entries:
///                  [count: u8]
///                  [chrom_idx: u8][start: u32 LE][strand: u8] × count
/// ─────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use rand::Rng;
use crate::error::{DendecError, Result};

const MAGIC: [u8; 4] = [0x44, 0x52, 0x46, 0x54]; // "DRFT"
const VERSION: u8 = 0x01;
pub const TABLE_SIZE: usize = 65_536; // 4^8
pub const KMER_LEN: usize = 8;

/// The pre-built lookup table embedded at compile time.
/// Compilation fails if data/table.bin does not exist — intentional.
static TABLE_BYTES: &[u8] = include_bytes!("../../data/table.bin");

/// A single genome coordinate from the lookup table.
#[derive(Clone, Debug)]
pub struct Coord {
    pub chrom_idx: u8, // index into the accession string table
    pub start: u32,    // 0-based start position (BED convention)
    pub strand: u8,    // 0 = forward (+), 1 = reverse (-)
}

/// Key used for the reverse index — uniquely identifies a coordinate.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CoordKey {
    pub chrom_idx: u8,
    pub start: u32,
    pub strand: u8,
}

impl From<&Coord> for CoordKey {
    fn from(c: &Coord) -> Self {
        CoordKey {
            chrom_idx: c.chrom_idx,
            start: c.start,
            strand: c.strand,
        }
    }
}

/// The loaded, in-memory refer table with both indices ready to use.
pub struct ReferTable {
    /// RefSeq accession strings in chrom_idx order.
    /// e.g. accessions[0] = "NC_000001.11"
    pub accessions: Vec<String>,

    /// Forward index: base-4 8-mer index → available genome coordinates.
    /// Used on the encode path.
    forward: Vec<Vec<Coord>>,

    /// Reverse index: coordinate key → base-4 8-mer index.
    /// Used on the decode path.
    reverse: HashMap<CoordKey, u16>,
}

impl ReferTable {
    /// Parse the embedded table.bin and build both indices.
    ///
    /// Called once at the start of refer_encode or refer_decode.
    /// Parsing is fast — a linear scan of ~3 MB of binary data.
    pub fn load() -> Result<Self> {
        let bytes = TABLE_BYTES;
        let mut cur = 0usize;

        // ── Magic ─────────────────────────────────────────────────────
        if bytes.len() < 7 || &bytes[0..4] != MAGIC {
            return Err(DendecError::ReferTableCorrupt);
        }
        cur += 4;

        // ── Version ───────────────────────────────────────────────────
        if bytes[cur] != VERSION {
            return Err(DendecError::ReferTableCorrupt);
        }
        cur += 1;

        // ── Chromosome count ──────────────────────────────────────────
        if cur + 2 > bytes.len() {
            return Err(DendecError::ReferTableCorrupt);
        }
        let chrom_count = u16::from_le_bytes(
            bytes[cur..cur + 2]
                .try_into()
                .map_err(|_| DendecError::ReferTableCorrupt)?,
        ) as usize;
        cur += 2;

        // ── Accession strings ─────────────────────────────────────────
        let mut accessions = Vec::with_capacity(chrom_count);
        for _ in 0..chrom_count {
            if cur >= bytes.len() {
                return Err(DendecError::ReferTableCorrupt);
            }
            let len = bytes[cur] as usize;
            cur += 1;
            if cur + len > bytes.len() {
                return Err(DendecError::ReferTableCorrupt);
            }
            let s = std::str::from_utf8(&bytes[cur..cur + len])
                .map_err(|_| DendecError::ReferTableCorrupt)?;
            accessions.push(s.to_string());
            cur += len;
        }

        // ── 65,536 8-mer entries ──────────────────────────────────────
        //
        // CRITICAL: the loop index must be usize, not u16.
        // TABLE_SIZE is 65,536 — exactly u16::MAX + 1. Casting it to u16
        // silently overflows to 0, producing an empty 0..0 range. The loop
        // would never execute and `forward` would remain empty, causing every
        // subsequent index access to panic. The cast to u16 is applied only
        // when inserting into `reverse`, where idx ≤ 65,535 and is safe.
        let mut forward: Vec<Vec<Coord>> = Vec::with_capacity(TABLE_SIZE);
        let mut reverse: HashMap<CoordKey, u16> =
            HashMap::with_capacity(TABLE_SIZE * 8);

        for idx in 0..TABLE_SIZE {
            if cur >= bytes.len() {
                return Err(DendecError::ReferTableCorrupt);
            }
            let count = bytes[cur] as usize;
            cur += 1;

            let mut coords = Vec::with_capacity(count);
            for _ in 0..count {
                if cur + 6 > bytes.len() {
                    return Err(DendecError::ReferTableCorrupt);
                }
                let chrom_idx = bytes[cur];
                let start = u32::from_le_bytes(
                    bytes[cur + 1..cur + 5]
                        .try_into()
                        .map_err(|_| DendecError::ReferTableCorrupt)?,
                );
                let strand = bytes[cur + 5];
                cur += 6;

                // Safe: idx is in 0..=65_535, which fits exactly in u16
                reverse.insert(
                    CoordKey { chrom_idx, start, strand },
                    idx as u16,
                );
                coords.push(Coord { chrom_idx, start, strand });
            }
            forward.push(coords);
        }

        Ok(ReferTable { accessions, forward, reverse })
    }

    // ── Index conversion ──────────────────────────────────────────────

    /// Convert an 8-mer byte slice to its base-4 index.
    ///
    /// Fixed mapping: A=0, T=1, G=2, C=3.
    /// Returns None for any non-ATGC byte (should not occur after
    /// chunk.rs validation, but handled defensively).
    pub fn kmer_to_index(kmer: &[u8]) -> Option<usize> {
        let mut idx = 0usize;
        for &b in kmer {
            idx <<= 2;
            idx |= match b {
                b'A' => 0,
                b'T' => 1,
                b'G' => 2,
                b'C' => 3,
                _ => return None,
            };
        }
        Some(idx)
    }

    /// Convert a base-4 index back to an 8-mer byte array.
    ///
    /// Fixed mapping: 0=A, 1=T, 2=G, 3=C. Inverse of kmer_to_index.
    pub fn index_to_kmer(mut idx: u16) -> [u8; KMER_LEN] {
        let mut kmer = [b'A'; KMER_LEN];
        for i in (0..KMER_LEN).rev() {
            kmer[i] = match idx & 0b11 {
                0 => b'A',
                1 => b'T',
                2 => b'G',
                3 => b'C',
                _ => unreachable!(),
            };
            idx >>= 2;
        }
        kmer
    }

    // ── Lookup ────────────────────────────────────────────────────────

    /// Forward lookup: 8-mer → a randomly selected genome coordinate.
    ///
    /// Random selection among the available coordinate options ensures that
    /// repeated 8-mers in the DNA produce varied coordinates in the BED
    /// output rather than mechanical repetition.
    ///
    /// Returns None only if the 8-mer has no coverage — should not occur
    /// with a complete table but handled defensively.
    pub fn lookup(&self, kmer: &[u8]) -> Option<Coord> {
        let idx = Self::kmer_to_index(kmer)?;
        let options = &self.forward[idx];
        if options.is_empty() {
            return None;
        }
        let pick = rand::thread_rng().gen_range(0..options.len());
        Some(options[pick].clone())
    }

    /// Reverse lookup: coordinate key → 8-mer byte array.
    ///
    /// Returns None if the coordinate is not in the index, which indicates
    /// a tampered or incompatible BED file.
    pub fn reverse_lookup(&self, key: &CoordKey) -> Option<[u8; KMER_LEN]> {
        let &idx = self.reverse.get(key)?;
        Some(Self::index_to_kmer(idx))
    }

    // ── Accession resolution ─────────────────────────────────────────

    /// Resolve a chromosome accession string to its chrom_idx.
    /// Linear search over at most 25 entries — effectively O(1).
    pub fn chrom_idx_for(&self, accession: &str) -> Option<u8> {
        self.accessions
            .iter()
            .position(|a| a == accession)
            .map(|i| i as u8)
    }

    /// Get the RefSeq accession string for a chrom_idx.
    pub fn accession_for(&self, chrom_idx: u8) -> Option<&str> {
        self.accessions.get(chrom_idx as usize).map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kmer_to_index_aaaaaaaa() {
        assert_eq!(ReferTable::kmer_to_index(b"AAAAAAAA"), Some(0));
    }

    #[test]
    fn test_kmer_to_index_cccccccc() {
        assert_eq!(ReferTable::kmer_to_index(b"CCCCCCCC"), Some(65535));
    }

    #[test]
    fn test_kmer_to_index_atgcatgc() {
        let idx = ReferTable::kmer_to_index(b"ATGCATGC").unwrap();
        assert!(idx < TABLE_SIZE);
    }

    #[test]
    fn test_index_to_kmer_roundtrip() {
        for idx in [0u16, 1, 100, 255, 1000, 32768, 65535] {
            let kmer = ReferTable::index_to_kmer(idx);
            let back = ReferTable::kmer_to_index(&kmer).unwrap();
            assert_eq!(back, idx as usize, "roundtrip failed for idx {}", idx);
        }
    }

    #[test]
    fn test_table_loads_without_panic() {
        let table = ReferTable::load().expect("table.bin failed to load");
        assert!(!table.accessions.is_empty());
        assert_eq!(table.forward.len(), TABLE_SIZE);
    }

    #[test]
    fn test_full_coverage() {
        let table = ReferTable::load().unwrap();
        let missing = table.forward.iter().filter(|e| e.is_empty()).count();
        assert_eq!(missing, 0, "{} 8-mers have no coverage", missing);
    }

    #[test]
    fn test_lookup_returns_coord() {
        let table = ReferTable::load().unwrap();
        let coord = table.lookup(b"ATGCGATC");
        assert!(coord.is_some(), "lookup returned None for valid 8-mer");
    }

    #[test]
    fn test_forward_reverse_roundtrip() {
        let table = ReferTable::load().unwrap();
        let kmer = b"ATGCGATC";
        let coord = table.lookup(kmer).expect("lookup failed");
        let key = CoordKey::from(&coord);
        let recovered = table.reverse_lookup(&key).expect("reverse lookup failed");
        assert_eq!(&recovered, kmer, "roundtrip did not recover original 8-mer");
    }

    #[test]
    fn test_all_kmers_roundtrip() {
        let table = ReferTable::load().unwrap();
        // Spot-check 256 evenly spaced indices across the full range
        for i in (0u16..=65535).step_by(256) {
            let kmer = ReferTable::index_to_kmer(i);
            let coord = table
                .lookup(&kmer)
                .unwrap_or_else(|| panic!("no coord for idx {}", i));
            let key = CoordKey::from(&coord);
            let recovered = table
                .reverse_lookup(&key)
                .unwrap_or_else(|| panic!("reverse lookup failed for idx {}", i));
            assert_eq!(recovered, kmer, "roundtrip failed for idx {}", i);
        }
    }
}


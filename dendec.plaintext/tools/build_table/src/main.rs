//! build_table — one-time offline tool to generate data/table.bin
//!
//! Walks chr1.fa.gz (and chr2.fa.gz if needed), slides an 8-mer window
//! across every real base position, and records genome coordinates for
//! all 65,536 possible 8-mers. The resulting binary is embedded into
//! the dendec binary at compile time via include_bytes!.
//!
//! OUTPUT FORMAT (data/table.bin)
//! ────────────────────────────────────────────────────────────────────
//!  Offset  Len   Field
//!  0       4     Magic: 0x44 0x52 0x46 0x54  ("DRFT")
//!  4       1     Version: 0x01
//!  5       2     Chromosome count (u16 LE) — number of accession strings
//!  7       var   Accession string table:
//!                  per entry: [len: u8][utf8 bytes]
//!  ?       var   65,536 sequential 8-mer entries (index 0 → 65535):
//!                  [count: u8]
//!                  [chrom_idx: u8][start: u32 LE][strand: u8]  × count
//! ────────────────────────────────────────────────────────────────────
//!
//! Chromosome index maps to the accession string table above.
//! Strand: 0 = forward (+), 1 = reverse (-)
//! Start positions are 0-based, matching BED convention.

use flate2::read::GzDecoder;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};

// ── Constants ────────────────────────────────────────────────────────

const MAGIC: [u8; 4] = [0x44, 0x52, 0x46, 0x54]; // "DRFT"
const VERSION: u8 = 0x01;
const TABLE_SIZE: usize = 65_536;  // 4^8 — all possible 8-mers
const MAX_PER_ENTRY: usize = 8;    // coordinate options per 8-mer
const KMER_LEN: usize = 8;

/// Chromosome sources in processing order.
/// chrom_idx maps directly into the accession string table written
/// into the binary header — table.rs uses the same index to recover
/// the full RefSeq accession for BED file output.
const SOURCES: &[(&str, u8, &str)] = &[
    ("NC_000001.11", 0, "chr1.fa.gz"),
    ("NC_000002.12", 1, "chr2.fa.gz"),
];

// ── Coordinate type ──────────────────────────────────────────────────

#[derive(Clone)]
struct Coord {
    chrom_idx: u8, // index into SOURCES accession table
    start: u32,    // 0-based start position
    strand: u8,    // 0 = +, 1 = -
}

// ── Core functions ───────────────────────────────────────────────────

/// Convert an 8-mer byte slice to a base-4 index.
/// Returns None if any byte is not A/T/G/C (e.g. N — unsequenced region).
fn base4_index(kmer: &[u8]) -> Option<usize> {
    debug_assert_eq!(kmer.len(), KMER_LEN);
    let mut idx = 0usize;
    for &b in kmer {
        idx <<= 2;
        idx |= match b {
            b'A' | b'a' => 0,
            b'T' | b't' => 1,
            b'G' | b'g' => 2,
            b'C' | b'c' => 3,
            _ => return None,
        };
    }
    Some(idx)
}

/// Reverse complement of an 8-mer.
fn reverse_complement(kmer: &[u8]) -> [u8; KMER_LEN] {
    let mut rc = [0u8; KMER_LEN];
    for (i, &b) in kmer.iter().rev().enumerate() {
        rc[i] = match b {
            b'A' | b'a' => b'T',
            b'T' | b't' => b'A',
            b'G' | b'g' => b'C',
            b'C' | b'c' => b'G',
            x => x,
        };
    }
    rc
}

/// Record a coordinate into the table if the entry still has room.
///
/// Returns (is_first, is_newly_saturated):
///   is_first          — true if this is the entry's first coordinate (newly filled)
///   is_newly_saturated — true if this coordinate caused the entry to reach MAX_PER_ENTRY
///
/// Both flags fire at most once per entry across the entire run, making
/// them safe to use as counters without double-counting.
fn record(table: &mut Vec<Vec<Coord>>, idx: usize, coord: Coord) -> (bool, bool) {
    let entry = &mut table[idx];
    if entry.len() < MAX_PER_ENTRY {
        let is_first = entry.is_empty();
        entry.push(coord);
        let is_newly_saturated = entry.len() == MAX_PER_ENTRY;
        return (is_first, is_newly_saturated);
    }
    (false, false)
}

// ── FASTA reader ─────────────────────────────────────────────────────

/// Read all sequence bytes from a gzipped FASTA file, skipping header lines.
/// Sequence is uppercased in-place to normalise soft-masked (lowercase) regions.
fn read_fasta_gz(path: &str) -> std::io::Result<Vec<u8>> {
    let f = File::open(path)?;
    let gz = GzDecoder::new(f);
    let reader = BufReader::new(gz);
    let mut sequence = Vec::with_capacity(256 * 1024 * 1024);

    for line in reader.lines() {
        let line = line?;
        if line.starts_with('>') {
            continue;
        }
        // Uppercase to handle soft-masked regions
        sequence.extend(line.trim().bytes().map(|b| b.to_ascii_uppercase()));
    }

    Ok(sequence)
}

// ── Serialiser ───────────────────────────────────────────────────────

/// Write the completed table to `output_path` in the documented binary format.
fn write_table(table: &[Vec<Coord>], output_path: &str) -> std::io::Result<()> {
    let mut out = File::create(output_path)?;

    // Magic + version
    out.write_all(&MAGIC)?;
    out.write_all(&[VERSION])?;

    // Chromosome accession string table
    let chrom_count = SOURCES.len() as u16;
    out.write_all(&chrom_count.to_le_bytes())?;
    for (accession, _, _) in SOURCES {
        let bytes = accession.as_bytes();
        out.write_all(&[bytes.len() as u8])?;
        out.write_all(bytes)?;
    }

    // 65,536 8-mer entries in base-4 index order
    for entry in table {
        out.write_all(&[entry.len() as u8])?;
        for coord in entry {
            out.write_all(&[coord.chrom_idx])?;
            out.write_all(&coord.start.to_le_bytes())?;
            out.write_all(&[coord.strand])?;
        }
    }

    Ok(())
}

// ── Main ─────────────────────────────────────────────────────────────

fn main() {
    let output_path = "../../data/table.bin";

    let mut table: Vec<Vec<Coord>> = vec![Vec::new(); TABLE_SIZE];
    let mut filled = 0usize;           // entries with at least one coordinate
    let mut fully_saturated = 0usize;  // entries with exactly MAX_PER_ENTRY coordinates

    'sources: for (accession, chrom_idx, path) in SOURCES {
        eprintln!("Processing {} ({})...", accession, path);

        let sequence = match read_fasta_gz(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("  ERROR reading {}: {}", path, e);
                std::process::exit(1);
            }
        };

        eprintln!("  {} bases loaded", sequence.len());

        let limit = sequence.len().saturating_sub(KMER_LEN - 1);

        for i in 0..limit {
            let kmer = &sequence[i..i + KMER_LEN];

            // Forward strand
            if let Some(idx) = base4_index(kmer) {
                let (is_first, is_saturated) = record(&mut table, idx, Coord {
                    chrom_idx: *chrom_idx,
                    start: i as u32,
                    strand: 0,
                });
                if is_first { filled += 1; }
                if is_saturated { fully_saturated += 1; }
            }

            // Reverse complement — different 8-mer, different table entry
            let rc = reverse_complement(kmer);
            if let Some(idx) = base4_index(&rc) {
                let (is_first, is_saturated) = record(&mut table, idx, Coord {
                    chrom_idx: *chrom_idx,
                    start: i as u32,
                    strand: 1,
                });
                if is_first { filled += 1; }
                if is_saturated { fully_saturated += 1; }
            }

            // Progress report every 10 million positions
            if i > 0 && i % 10_000_000 == 0 {
                eprintln!(
                    "  position {:>12}  filled {}/{}  saturated {}/{}",
                    i, filled, TABLE_SIZE, fully_saturated, TABLE_SIZE
                );
            }

            // Early exit only when every single entry is fully saturated
            if fully_saturated == TABLE_SIZE {
                eprintln!(
                    "  All {} entries saturated at position {}. Stopping early.",
                    TABLE_SIZE, i
                );
                break 'sources;
            }
        }

        eprintln!(
            "  Finished {}  filled {}/{}  saturated {}/{}",
            accession, filled, TABLE_SIZE, fully_saturated, TABLE_SIZE
        );
    }

    // ── Coverage report ──────────────────────────────────────────────

    let missing: Vec<usize> = table.iter().enumerate()
        .filter(|(_, v)| v.is_empty())
        .map(|(i, _)| i)
        .collect();

    let partial: Vec<usize> = table.iter().enumerate()
        .filter(|(_, v)| !v.is_empty() && v.len() < MAX_PER_ENTRY)
        .map(|(i, _)| i)
        .collect();

    if missing.is_empty() {
        eprintln!("\nAll 65,536 8-mers covered.");
    } else {
        eprintln!(
            "\nWARNING: {} 8-mers have no coverage.",
            missing.len()
        );
        for &idx in missing.iter().take(5) {
            eprintln!("  Missing index: {}", idx);
        }
    }

    if !partial.is_empty() {
        eprintln!(
            "  {} 8-mers have partial coverage (fewer than {} coordinates).",
            partial.len(), MAX_PER_ENTRY
        );
    }

    // ── Write output ─────────────────────────────────────────────────

    std::fs::create_dir_all("../../data").unwrap_or(());

    match write_table(&table, output_path) {
        Ok(_) => {
            let size = std::fs::metadata(output_path)
                .map(|m| m.len())
                .unwrap_or(0);
            eprintln!(
                "Written to {}  ({:.1} KB)",
                output_path,
                size as f64 / 1024.0
            );
        }
        Err(e) => {
            eprintln!("ERROR writing table: {}", e);
            std::process::exit(1);
        }
    }
}


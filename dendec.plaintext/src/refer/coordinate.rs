/// refer/coordinate.rs — BED file read and write
///
/// Owns the complete BED format for dendec refer output. The format is
/// deliberately identical to standard genomics BED files so that the output
/// is indistinguishable from routine bioinformatics annotation work.
///
/// BED FORMAT USED
/// ───────────────────────────────────────────────────────────────────────
///  ##dendec-refer v0.1.0
///  ##assembly GCF_000001405.40 hg38
///  ##chunk_size 8
///  ##dna_length 168432
///  ##chunk_count 21054
///  NC_000001.11  883401  883409  chunk_00000000  0  +
///  NC_000007.14  553084  553092  chunk_00000001  0  -
///
/// Column layout:
///   1  Chromosome accession (RefSeq format)
///   2  Start position (0-based, BED convention)
///   3  End position (start + 8, exclusive)
///   4  Chunk name (chunk_ + zero-padded 8-digit index)
///   5  Score (always 0 — unused, present for BED compliance)
///   6  Strand (+ or -)

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use crate::error::{DendecError, Result};

const REFER_VERSION: &str = "0.1.0";
const ASSEMBLY: &str = "GCF_000001405.40 hg38";
const CHUNK_SIZE: usize = 8;

/// A single parsed record from a dendec-refer BED file.
pub struct BedRecord {
    /// RefSeq accession string for the chromosome.
    pub accession: String,
    /// 0-based start position of the 8-mer in the chromosome.
    pub start: u32,
    /// Strand: 0 = forward (+), 1 = reverse (-).
    pub strand: u8,
    /// Chunk index — determines reassembly order.
    pub chunk_idx: usize,
}

/// Metadata recovered from the ## header lines of a BED file.
pub struct BedHeader {
    /// Total length of the original DNA string in bases.
    pub dna_length: usize,
    /// Number of chunks (BED data lines) expected.
    /// This chunk_count is preserved for future rollouts and testing purposes only. dead code 
    #[allow(dead_code)]
    pub chunk_count: usize,
}

/// Write a dendec-refer BED file to `path`.
///
/// `records` is a slice of (accession, start, strand) tuples in chunk
/// order. `dna_length` is the total character count of the source DNA
/// string, stored in the header for defensive trimming on decode.
pub fn write_bed(
    path: &Path,
    records: &[(String, u32, u8)],
    dna_length: usize,
) -> Result<()> {
    let file = File::create(path).map_err(DendecError::Io)?;
    let mut w = BufWriter::new(file);

    // Standard ## comment headers — identical in style to VCF/GFF
    writeln!(w, "##dendec-refer v{}", REFER_VERSION).map_err(DendecError::Io)?;
    writeln!(w, "##assembly {}", ASSEMBLY).map_err(DendecError::Io)?;
    writeln!(w, "##chunk_size {}", CHUNK_SIZE).map_err(DendecError::Io)?;
    writeln!(w, "##dna_length {}", dna_length).map_err(DendecError::Io)?;
    writeln!(w, "##chunk_count {}", records.len()).map_err(DendecError::Io)?;

    for (i, (accession, start, strand)) in records.iter().enumerate() {
        let end = start + CHUNK_SIZE as u32;
        let strand_char = if *strand == 0 { '+' } else { '-' };
        writeln!(
            w,
            "{}\t{}\t{}\tchunk_{:08}\t0\t{}",
            accession, start, end, i, strand_char
        )
        .map_err(DendecError::Io)?;
    }

    Ok(())
}

/// Parse a dendec-refer BED file from `path`.
///
/// Returns the parsed header metadata and a list of records sorted by
/// chunk index. Sorting is defensive — file order should already be
/// correct, but an out-of-order BED file will still decode correctly.
pub fn read_bed(path: &Path) -> Result<(BedHeader, Vec<BedRecord>)> {
    let file = File::open(path).map_err(DendecError::Io)?;
    let reader = BufReader::new(file);

    let mut dna_length = 0usize;
    let mut chunk_count = 0usize;
    let mut records: Vec<BedRecord> = Vec::new();

    for raw in reader.lines() {
        let line = raw.map_err(DendecError::Io)?;
        let line = line.trim();

        // ── Header lines ──────────────────────────────────────────────
        if line.starts_with("##dna_length") {
            dna_length = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            continue;
        }
        if line.starts_with("##chunk_count") {
            chunk_count = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            continue;
        }
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        // ── Data lines ────────────────────────────────────────────────
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 6 {
            return Err(DendecError::ReferInvalidBed(format!(
                "expected 6 tab-separated columns, got {}: {:?}",
                cols.len(),
                line
            )));
        }

        let accession = cols[0].to_string();

        let start: u32 = cols[1].parse().map_err(|_| {
            DendecError::ReferInvalidBed(format!(
                "invalid start coordinate '{}' in line: {}",
                cols[1], line
            ))
        })?;

        let strand: u8 = match cols[5] {
            "+" => 0,
            "-" => 1,
            other => {
                return Err(DendecError::ReferInvalidBed(format!(
                    "invalid strand '{}': expected '+' or '-'",
                    other
                )))
            }
        };

        let chunk_idx: usize = cols[3]
            .strip_prefix("chunk_")
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                DendecError::ReferInvalidBed(format!(
                    "invalid chunk name '{}': expected chunk_NNNNNNNN",
                    cols[3]
                ))
            })?;

        records.push(BedRecord {
            accession,
            start,
            strand,
            chunk_idx,
        });
    }

    // Defensive sort by chunk index
    records.sort_by_key(|r| r.chunk_idx);

    Ok((BedHeader { dna_length, chunk_count }, records))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_write_and_read_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.bed");

        let records = vec![
            ("NC_000001.11".to_string(), 883401u32, 0u8),
            ("NC_000001.11".to_string(), 19823u32,  1u8),
            ("NC_000001.11".to_string(), 28401u32,  0u8),
        ];

        write_bed(&path, &records, 24).unwrap();

        let (header, parsed) = read_bed(&path).unwrap();
        assert_eq!(header.dna_length, 24);
        assert_eq!(header.chunk_count, 3);
        assert_eq!(parsed.len(), 3);

        assert_eq!(parsed[0].accession, "NC_000001.11");
        assert_eq!(parsed[0].start, 883401);
        assert_eq!(parsed[0].strand, 0);
        assert_eq!(parsed[0].chunk_idx, 0);

        assert_eq!(parsed[1].strand, 1);
        assert_eq!(parsed[1].chunk_idx, 1);
    }

    #[test]
    fn test_missing_columns_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.bed");
        std::fs::write(&path, "NC_000001.11\t883401\n").unwrap();
        assert!(read_bed(&path).is_err());
    }

    #[test]
    fn test_invalid_strand_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad_strand.bed");
        std::fs::write(
            &path,
            "NC_000001.11\t883401\t883409\tchunk_00000000\t0\t?\n",
        )
        .unwrap();
        assert!(read_bed(&path).is_err());
    }

    #[test]
    fn test_invalid_chunk_name_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad_chunk.bed");
        std::fs::write(
            &path,
            "NC_000001.11\t883401\t883409\tbadname\t0\t+\n",
        )
        .unwrap();
        assert!(read_bed(&path).is_err());
    }

    #[test]
    fn test_out_of_order_records_sorted() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("unordered.bed");

        // Write chunk_00000001 before chunk_00000000
        std::fs::write(
            &path,
            "##dna_length 16\n\
             ##chunk_count 2\n\
             NC_000001.11\t883401\t883409\tchunk_00000001\t0\t+\n\
             NC_000001.11\t19823\t19831\tchunk_00000000\t0\t-\n",
        )
        .unwrap();

        let (_, records) = read_bed(&path).unwrap();
        assert_eq!(records[0].chunk_idx, 0);
        assert_eq!(records[1].chunk_idx, 1);
    }
}


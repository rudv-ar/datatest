/// refer/mod.rs — Orchestration for dendec refer
///
/// Exposes two public functions that main.rs calls directly:
///
///   refer_encode(from, to) — .dna file → .bed file
///   refer_decode(from, to) — .bed file → .dna file
///
/// Both are fully offline. The embedded lookup table handles all
/// coordinate translation without any network access.

pub mod table;
mod chunk;
mod coordinate;
mod reverse;

use std::path::PathBuf;

use crate::error::{DendecError, Result};
use chunk::{split_into_kmers, reassemble};
use coordinate::{read_bed, write_bed};
use table::{CoordKey, ReferTable};

/// Convert a .dna file into a genomic coordinate BED file.
///
/// Reads the flat ATGC string produced by `dendec encode`, chunks it
/// into successive 8-mers, maps each to a real hg38 coordinate via the
/// embedded lookup table, and writes a standard BED file. Fully offline.
pub fn refer_encode(from: PathBuf, to: PathBuf) -> Result<()> {
    // ── Load table ────────────────────────────────────────────────────
    eprintln!("Loading reference table...");
    let table = ReferTable::load()?;

    // ── Read DNA file ─────────────────────────────────────────────────
    let raw = std::fs::read_to_string(&from).map_err(DendecError::Io)?;

    // Strip any whitespace (grouping spaces, newlines) the encode step
    // may have introduced — same defensive strip as dendec decode uses.
    let dna: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    let dna_bytes = dna.as_bytes();
    let dna_length = dna_bytes.len();

    eprintln!("  Read {} bases from {}", dna_length, from.display());

    // ── Split into 8-mers ─────────────────────────────────────────────
    let kmers = split_into_kmers(dna_bytes)?;
    let chunk_count = kmers.len();

    eprintln!("  Mapping {} 8-mers to genome coordinates...", chunk_count);

    // ── Lookup each 8-mer ─────────────────────────────────────────────
    let mut records: Vec<(String, u32, u8)> = Vec::with_capacity(chunk_count);

    for (i, kmer) in kmers.iter().enumerate() {
        let coord = table
            .lookup(kmer)
            .ok_or(DendecError::ReferChunkNotFound { chunk: i })?;

        let accession = table
            .accession_for(coord.chrom_idx)
            .ok_or(DendecError::ReferTableCorrupt)?
            .to_string();

        records.push((accession, coord.start, coord.strand));
    }

    // ── Write BED file ────────────────────────────────────────────────
    write_bed(&to, &records, dna_length)?;

    eprintln!(
        "  Written {} chunks → {}",
        chunk_count,
        to.display()
    );

    Ok(())
}

/// Reconstruct a .dna file from a genomic coordinate BED file.
///
/// Parses the BED file, resolves each coordinate to its original 8-mer
/// via the embedded reverse index, reassembles the 8-mers in chunk
/// order, and writes the flat ATGC string. Fully offline.
pub fn refer_decode(from: PathBuf, to: PathBuf) -> Result<()> {
    // ── Load table ────────────────────────────────────────────────────
    eprintln!("Loading reference table...");
    let table = ReferTable::load()?;

    // ── Read BED file ─────────────────────────────────────────────────
    let (header, records) = read_bed(&from)?;

    eprintln!(
        "  Read {} chunks from {}",
        records.len(),
        from.display()
    );

    // ── Reverse lookup each coordinate ────────────────────────────────
    let mut kmers: Vec<[u8; 8]> = Vec::with_capacity(records.len());

    for record in &records {
        // Resolve accession string → chrom_idx
        let chrom_idx = table
            .chrom_idx_for(&record.accession)
            .ok_or_else(|| DendecError::ReferAssemblyMismatch {
                expected: "known hg38 accession".to_string(),
                got: record.accession.clone(),
            })?;

        let key = CoordKey {
            chrom_idx,
            start: record.start,
            strand: record.strand,
        };

        // O(1) reverse lookup → original 8-mer
        let kmer = table
            .reverse_lookup(&key)
            .ok_or(DendecError::ReferChunkNotFound { chunk: record.chunk_idx })?;

        kmers.push(kmer);
    }

    // ── Reassemble and write ──────────────────────────────────────────
    let mut dna = reassemble(&kmers);

    // Defensive trim: if the original DNA length was recorded in the header,
    // truncate to that length. In practice the lengths should always match.
    if header.dna_length > 0 && dna.len() > header.dna_length {
        dna.truncate(header.dna_length);
    }

    std::fs::write(&to, dna.as_bytes()).map_err(DendecError::Io)?;

    eprintln!(
        "  Recovered {} bases → {}",
        dna.len(),
        to.display()
    );

    Ok(())
}


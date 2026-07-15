//! Coverage meter — histogram over (glyph/primitive × tier × difficulty).
//!
//! Counts all 23 manifest glyphs across accepted programs and flags any glyph
//! below a floor as a "hole" (design §3: the CI-failing coverage meter, so rare
//! primitives `| ( $ ; , '` are not starved). Emits `coverage.json`.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::GLYPHS;

/// One accepted program to meter: (source, tier, difficulty).
pub struct Prog<'a> {
    pub src: &'a str,
    pub tier: u8,
    pub difficulty: u32,
}

/// The serializable coverage report.
#[derive(Serialize, Debug)]
pub struct CoverageReport {
    pub total_programs: usize,
    pub floor: u64,
    /// glyph -> total occurrence count across all accepted programs.
    pub per_glyph: BTreeMap<String, u64>,
    /// tier -> number of programs.
    pub per_tier: BTreeMap<String, u64>,
    /// difficulty -> number of programs.
    pub per_difficulty: BTreeMap<String, u64>,
    /// glyph -> (tier -> occurrence count).
    pub glyph_by_tier: BTreeMap<String, BTreeMap<String, u64>>,
    /// glyphs whose total count is below `floor`.
    pub holes: Vec<String>,
}

impl CoverageReport {
    pub fn has_holes(&self) -> bool {
        !self.holes.is_empty()
    }
}

/// Count glyph occurrences in one parsed program (recursing into quotes). Uses
/// the manifest glyph map so it cannot drift from the canonical primitive set.
pub fn count_glyphs(src: &str) -> BTreeMap<char, u64> {
    let mut m: BTreeMap<char, u64> = BTreeMap::new();
    if let Ok(prog) = mtl_syntax::parse(src) {
        walk(&prog, &mut m);
    }
    m
}

fn walk(prog: &[mtl_syntax::Word], m: &mut BTreeMap<char, u64>) {
    for w in prog {
        match w {
            mtl_syntax::Word::Prim(p) => {
                let g = mtl_syntax::manifest::meta_of(*p).glyph;
                *m.entry(g).or_insert(0) += 1;
            }
            mtl_syntax::Word::PushQuote(q) => walk(q, m),
            _ => {}
        }
    }
}

/// Meter the accepted programs and produce the coverage report.
pub fn measure(progs: &[Prog], floor: u64) -> CoverageReport {
    let mut per_glyph: BTreeMap<String, u64> = BTreeMap::new();
    for &g in GLYPHS.iter() {
        per_glyph.insert(g.to_string(), 0);
    }
    let mut per_tier: BTreeMap<String, u64> = BTreeMap::new();
    let mut per_difficulty: BTreeMap<String, u64> = BTreeMap::new();
    let mut glyph_by_tier: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();

    for p in progs {
        *per_tier.entry(p.tier.to_string()).or_insert(0) += 1;
        *per_difficulty.entry(p.difficulty.to_string()).or_insert(0) += 1;
        for (g, c) in count_glyphs(p.src) {
            *per_glyph.entry(g.to_string()).or_insert(0) += c;
            *glyph_by_tier
                .entry(g.to_string())
                .or_default()
                .entry(p.tier.to_string())
                .or_insert(0) += c;
        }
    }

    let mut holes: Vec<String> = per_glyph
        .iter()
        .filter(|(_, &c)| c < floor)
        .map(|(g, _)| g.clone())
        .collect();
    holes.sort();

    CoverageReport {
        total_programs: progs.len(),
        floor,
        per_glyph,
        per_tier,
        per_difficulty,
        glyph_by_tier,
        holes,
    }
}

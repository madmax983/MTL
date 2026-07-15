//! The coverage meter counts glyphs/tiers/difficulty correctly and flags a
//! deliberately-omitted glyph as a hole.

use mtl_datagen::coverage::{count_glyphs, measure, Prog};

#[test]
fn counts_glyph_occurrences() {
    // 3*7+  -> one '*', one '+'
    let m = count_glyphs("3*7+");
    assert_eq!(m.get(&'*'), Some(&1));
    assert_eq!(m.get(&'+'), Some(&1));
    assert_eq!(m.get(&'-'), None);

    // :*:*  -> two ':' and two '*'  (recurses correctly, flat here)
    let m = count_glyphs(":*:*");
    assert_eq!(m.get(&':'), Some(&2));
    assert_eq!(m.get(&'*'), Some(&2));

    // glyphs nested inside a quote are counted: [1+]! -> one '+', one '!'
    let m = count_glyphs("[1+]!");
    assert_eq!(m.get(&'+'), Some(&1));
    assert_eq!(m.get(&'!'), Some(&1));
}

#[test]
fn meters_tiers_difficulty_and_flags_holes() {
    let progs = vec![
        Prog {
            src: "3*7+",
            tier: 0,
            difficulty: 0,
        },
        Prog {
            src: "0[+](",
            tier: 2,
            difficulty: 1,
        },
        Prog {
            src: ":*",
            tier: 0,
            difficulty: 0,
        },
    ];
    // Floor of 1: every glyph appearing >= once is covered; the rest are holes.
    let rep = measure(&progs, 1);

    assert_eq!(rep.total_programs, 3);
    assert_eq!(rep.per_tier.get("0"), Some(&2));
    assert_eq!(rep.per_tier.get("2"), Some(&1));
    assert_eq!(rep.per_difficulty.get("0"), Some(&2));
    assert_eq!(rep.per_difficulty.get("1"), Some(&1));

    // '*' appears in "3*7+" and ":*" => 2 occurrences.
    assert_eq!(rep.per_glyph.get("*"), Some(&2));
    // '+' appears in "3*7+" and "0[+](" => 2.
    assert_eq!(rep.per_glyph.get("+"), Some(&2));
    // '(' (fold) appears once in "0[+](".
    assert_eq!(rep.per_glyph.get("("), Some(&1));
    // glyph_by_tier: '(' only in the tier-2 program.
    assert_eq!(
        rep.glyph_by_tier.get("(").and_then(|t| t.get("2")),
        Some(&1)
    );

    // '|' (linrec) is deliberately never used -> it must be a hole.
    assert_eq!(rep.per_glyph.get("|"), Some(&0));
    assert!(
        rep.holes.contains(&"|".to_string()),
        "omitted glyph not flagged"
    );
    assert!(rep.has_holes());
    // '*' is NOT a hole at floor 1.
    assert!(!rep.holes.contains(&"*".to_string()));
}

#[test]
fn floor_raises_more_holes() {
    let progs = vec![Prog {
        src: "3*7+",
        tier: 0,
        difficulty: 0,
    }];
    // At floor 2, the single '*' (count 1) is now a hole.
    let rep = measure(&progs, 2);
    assert!(rep.holes.contains(&"*".to_string()));
}

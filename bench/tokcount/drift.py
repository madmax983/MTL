#!/usr/bin/env python3
"""drift.py — tokenizer-drift guard for the MTL token-economy thesis.

Tokenizer vocabularies drift across model generations, and that drift silently
moves the whole token-economy thesis: pxpipe measured a ~4x "knee" shift in the
token economy between model generations purely from vocab changes. To keep the
MTL numbers honest we check in a deterministic *profile* of token counts and
recompute it in CI; any change is surfaced (non-blocking) so it is a conscious,
reviewed event rather than a silent one.

The profile records, per tiktoken encoding (both ENCODINGS):
  - glyphs:  the token count of every individual MTL glyph/primitive, each
             counted as an isolated string (a fixed, checked-in glyph list).
  - corpus:  the token count of every corpus program the tokcount harness
             already enumerates (reuses report.load_tasks() / tasks.json).
It also pins the tiktoken version and the encoding names.

Usage:
    python3 bench/tokcount/drift.py --check    # default; diff vs checked-in, exit nonzero on drift
    python3 bench/tokcount/drift.py --update    # (re)write token_profile.json
    python3 bench/tokcount/drift.py --write      # alias of --update

Network graceful degradation: tiktoken lazily downloads its vocab files on first
use. If an encoder cannot load (the common CI failure), the counts degrade to
None and we do NOT report false drift — --check exits 0 with a clear SKIPPED
message. Only real count changes with successfully-loaded encoders count as drift.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

# Make `import tokcount` / `import report` work regardless of cwd.
_HERE = Path(__file__).resolve().parent          # bench/tokcount
_BENCH = _HERE.parent                            # bench
if str(_BENCH) not in sys.path:
    sys.path.insert(0, str(_BENCH))
if str(_HERE) not in sys.path:
    sys.path.insert(0, str(_HERE))

from tokcount.tokcount import ENCODINGS, count, count_file, encoder_error  # noqa: E402
import report  # noqa: E402  (reuse the corpus enumeration; no duplicate walk)

PROFILE_PATH = _HERE / "token_profile.json"

# Fixed, checked-in MTL glyph/primitive list. Each is counted as an isolated
# string. Kept as an explicit literal list (not derived) so the drift guard has
# a stable, human-auditable input that does not move with the grammar.
GLYPHS = [
    "[", "]", ":", "_", "~", "@", "^", "!", ",", ";", "'", "+", "-", "*",
    "/", "%", "=", "<", "?", "&", ".", "|", ">", "(", "$",
]


def _tiktoken_version() -> str:
    try:
        import tiktoken
        return getattr(tiktoken, "__version__", "unknown")
    except ImportError:
        return "NOT INSTALLED"


def encoders_unavailable() -> list[tuple[str, str]]:
    """Return [(encoding, error)] for any encoding whose encoder failed to load.

    Probes each encoder once (via a trivial count) so encoder_error() is populated.
    """
    count("")  # force a load attempt for every encoding
    out: list[tuple[str, str]] = []
    for enc in ENCODINGS:
        err = encoder_error(enc)
        if err:
            out.append((enc, err))
    return out


def compute_profile() -> dict:
    """Compute the deterministic token profile.

    Assumes encoders are available (call encoders_unavailable() first to guard).
    """
    glyphs: dict[str, dict[str, int | None]] = {}
    for g in GLYPHS:
        glyphs[g] = count(g)

    # Per-corpus aggregate: token count of every corpus program the harness
    # already knows about. Enumerate every file referenced by tasks.json across
    # all tasks/variants (reuses report.load_tasks()); key by its relative path
    # so the diff is stable and points at an exact program.
    corpus: dict[str, dict[str, int | None]] = {}
    for task in report.load_tasks():
        for rel in task.get("files", {}).values():
            path = _BENCH / rel
            if path.exists():
                corpus[rel] = count_file(path)

    return {
        "tiktoken_version": _tiktoken_version(),
        "encodings": list(ENCODINGS),
        "glyphs": glyphs,
        "corpus": corpus,
    }


def write_profile() -> int:
    unavailable = encoders_unavailable()
    if unavailable:
        reasons = "; ".join(f"{enc}: {err}" for enc, err in unavailable)
        print(f"REFUSING TO WRITE: tokenizer unavailable ({reasons})", file=sys.stderr)
        print("A profile written with unavailable encoders would bake in null "
              "counts. Regenerate where tiktoken can load its vocab.", file=sys.stderr)
        return 2
    profile = compute_profile()
    PROFILE_PATH.write_text(
        json.dumps(profile, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(f"[drift.py] wrote {PROFILE_PATH}")
    print(f"[drift.py] tiktoken_version={profile['tiktoken_version']} "
          f"encodings={','.join(profile['encodings'])} "
          f"glyphs={len(profile['glyphs'])} corpus_programs={len(profile['corpus'])}")
    return 0


def _diff_section(name: str, old: dict, new: dict) -> list[str]:
    """Human-readable diff of a {key: {enc: count}} section."""
    lines: list[str] = []
    keys = sorted(set(old) | set(new))
    for key in keys:
        o = old.get(key)
        n = new.get(key)
        if o is None:
            lines.append(f"  [{name}] {key!r}: ADDED -> {n}")
            continue
        if n is None:
            lines.append(f"  [{name}] {key!r}: REMOVED (was {o})")
            continue
        for enc in ENCODINGS:
            ov = o.get(enc)
            nv = n.get(enc)
            if ov != nv:
                lines.append(f"  [{name}] {key!r} / {enc}: {ov} -> {nv}")
    return lines


def check_profile() -> int:
    if not PROFILE_PATH.exists():
        print(f"NO PROFILE: {PROFILE_PATH} is missing. Generate it with:\n"
              f"    python3 bench/tokcount/drift.py --update", file=sys.stderr)
        return 2

    # Graceful degradation: never report false drift when encoders can't load.
    unavailable = encoders_unavailable()
    if unavailable:
        reasons = "; ".join(f"{enc}: {err}" for enc, err in unavailable)
        print(f"SKIPPED: tokenizer unavailable ({reasons})")
        print("Cannot recompute the token profile without loaded encoders; "
              "treating as no-drift so a blocked vocab download never fails this check.")
        return 0

    checked_in = json.loads(PROFILE_PATH.read_text(encoding="utf-8"))
    current = compute_profile()

    diffs: list[str] = []

    old_ver = checked_in.get("tiktoken_version")
    new_ver = current.get("tiktoken_version")
    if old_ver != new_ver:
        diffs.append(f"  [tiktoken_version]: {old_ver} -> {new_ver}")

    old_encs = checked_in.get("encodings")
    new_encs = current.get("encodings")
    if old_encs != new_encs:
        diffs.append(f"  [encodings]: {old_encs} -> {new_encs}")

    diffs += _diff_section("glyph", checked_in.get("glyphs", {}), current.get("glyphs", {}))
    diffs += _diff_section("corpus", checked_in.get("corpus", {}), current.get("corpus", {}))

    if diffs:
        print("TOKENIZER DRIFT DETECTED — checked-in token_profile.json no longer matches:")
        print(f"  tiktoken_version (checked-in): {old_ver}  (current: {new_ver})")
        for line in diffs:
            print(line)
        print()
        print("If this drift is expected (new tiktoken, changed corpus), review it and "
              "regenerate:\n    python3 bench/tokcount/drift.py --update")
        return 1

    print("no drift — token profile matches the checked-in "
          f"token_profile.json (tiktoken {new_ver}, "
          f"encodings {','.join(new_encs or [])}).")
    return 0


def _cli(argv: list[str]) -> int:
    mode = "--check"
    for a in argv[1:]:
        if a in ("--check", "--update", "--write"):
            mode = a
        else:
            print(f"unknown argument: {a}", file=sys.stderr)
            print(__doc__, file=sys.stderr)
            return 2
    if mode in ("--update", "--write"):
        return write_profile()
    return check_profile()


if __name__ == "__main__":
    raise SystemExit(_cli(sys.argv))

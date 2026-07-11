"""tokcount.tokcount — core token-counting module.

Counts tokens of a string or file under the OpenAI tiktoken encodings used
as a public proxy for LLM tokenization: o200k_base (GPT-4o family) and
cl100k_base (GPT-4 / GPT-3.5 family).

Design goals:
  - dependency-light: only `tiktoken`.
  - lazy, cached encoder loading; a failed encoder degrades gracefully to a
    `None` count plus a remembered error string (never fakes a number).

Note: the Claude tokenizer has no pinned public implementation; see the
`count_claude` stub in bench/README.md for the intended API-based approach.
"""

from __future__ import annotations

import sys
from pathlib import Path

# Public proxy encodings. Order is significant for report columns.
ENCODINGS = ["o200k_base", "cl100k_base"]

# Cache of loaded encoders and load errors, keyed by encoding name.
_ENCODER_CACHE: dict[str, object] = {}
_ENCODER_ERRORS: dict[str, str] = {}

_MISSING_TIKTOKEN_MSG = (
    "tiktoken is not installed. Install the harness dependencies with:\n"
    "    pip3 install -r bench/tokcount/requirements.txt"
)


def _get_encoder(name: str):
    """Lazily load and cache a tiktoken encoder.

    Returns the encoder, or None if it could not be loaded (import failure or
    vocab-download failure). The error string is stored in _ENCODER_ERRORS.
    """
    if name in _ENCODER_CACHE:
        return _ENCODER_CACHE[name]
    if name in _ENCODER_ERRORS:
        return None
    try:
        import tiktoken
    except ImportError as exc:  # pragma: no cover - environment dependent
        _ENCODER_ERRORS[name] = f"tiktoken import failed: {exc}"
        return None
    try:
        enc = tiktoken.get_encoding(name)
    except Exception as exc:  # noqa: BLE001 - report any load failure verbatim
        # Most commonly a blocked vocab download from openaipublic.blob.core.windows.net.
        _ENCODER_ERRORS[name] = f"{type(exc).__name__}: {exc}"
        return None
    _ENCODER_CACHE[name] = enc
    return enc


def encoder_error(name: str) -> str | None:
    """Return the load-error string for an encoding, if it failed to load."""
    return _ENCODER_ERRORS.get(name)


def count(text: str) -> dict[str, int | None]:
    """Count tokens of `text` under every encoding in ENCODINGS.

    Returns a dict mapping encoding name -> token count, or None for that
    encoding if its encoder could not be loaded (the reason is retrievable via
    `encoder_error(name)`).
    """
    result: dict[str, int | None] = {}
    for name in ENCODINGS:
        enc = _get_encoder(name)
        if enc is None:
            result[name] = None
        else:
            result[name] = len(enc.encode(text))
    return result


def count_file(path) -> dict[str, int | None]:
    """Count tokens of a file's contents.

    A single trailing newline is stripped so that an editor-added final
    newline does not inflate the count; this is applied uniformly to every
    variant so cross-variant comparisons stay fair.
    """
    text = Path(path).read_text(encoding="utf-8")
    if text.endswith("\n"):
        text = text[:-1]
    return count(text)


def _cli(argv: list[str]) -> int:
    # Determine the input text: explicit arg (path or literal), else stdin.
    if len(argv) >= 2:
        arg = argv[1]
        p = Path(arg)
        if p.exists():
            text = p.read_text(encoding="utf-8")
            if text.endswith("\n"):
                text = text[:-1]
            source = f"file: {arg}"
        else:
            text = arg
            source = "string (literal arg)"
    else:
        text = sys.stdin.read()
        if text.endswith("\n"):
            text = text[:-1]
        source = "string (stdin)"

    # Fail fast with an actionable message if tiktoken is entirely missing.
    try:
        import tiktoken  # noqa: F401
    except ImportError:
        print(_MISSING_TIKTOKEN_MSG, file=sys.stderr)
        return 2

    counts = count(text)
    print(f"# tokcount — {source}")
    print(f"# input length: {len(text)} chars")
    print()
    print(f"{'encoding':<14} {'tokens':>8} {'token-list-len':>16}")
    print(f"{'-' * 14} {'-' * 8:>8} {'-' * 16:>16}")
    for name in ENCODINGS:
        n = counts[name]
        if n is None:
            err = encoder_error(name) or "unavailable"
            print(f"{name:<14} {'UNAVAIL':>8} {'':>16}  # {err}")
        else:
            # token-list-len == token count for a single encode; shown to make
            # the "these are the tokens" relationship explicit.
            print(f"{name:<14} {n:>8} {n:>16}")
    return 0


if __name__ == "__main__":
    raise SystemExit(_cli(sys.argv))

"""rle — run-length encoding. Idiomatic minimal Python reference (T_v0.2).

Output format: each maximal run of a character c of length n is emitted as
c followed by the decimal count n. "aaabbc" -> "a3b2c1".

Runnable. Tested:
    >>> rle("aaabbc")
    'a3b2c1'
    >>> rle("")
    ''
    >>> rle("abc")
    'a1b1c1'
    >>> rle("aaaaaaaaaaaa")   # 12 a's -> count is multi-digit
    'a12'
"""


def rle(s):
    if not s:
        return ""
    out = []
    prev, n = s[0], 1
    for c in s[1:]:
        if c == prev:
            n += 1
        else:
            out.append(prev + str(n))
            prev, n = c, 1
    out.append(prev + str(n))
    return "".join(out)


if __name__ == "__main__":
    for arg in ["aaabbc", "", "abc", "aaaaaaaaaaaa", "wwwwaaadexxxxxx"]:
        print(f"rle({arg!r}) = {rle(arg)!r}")

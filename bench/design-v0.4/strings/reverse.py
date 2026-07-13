"""reverse — reverse a string. Idiomatic minimal Python reference (T_v0.2).

Runnable. Tested:
    >>> reverse("abc")
    'cba'
    >>> reverse("")
    ''
    >>> reverse("racecar")
    'racecar'
    >>> reverse("a")
    'a'
"""


def reverse(s):
    return s[::-1]


if __name__ == "__main__":
    for arg in ["abc", "", "racecar", "a", "hello world"]:
        print(f"reverse({arg!r}) = {reverse(arg)!r}")

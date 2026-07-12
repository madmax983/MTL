def palindrome_number(n):
    rev = 0
    m = n
    while m > 0:
        rev = rev * 10 + m % 10
        m //= 10
    return 1 if rev == n else 0

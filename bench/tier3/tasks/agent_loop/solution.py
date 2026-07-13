def solve():
    s = read_state()
    while not done(s):
        s = step(s)
    return s

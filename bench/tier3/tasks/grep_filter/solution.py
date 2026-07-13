def solve():
    for line in read_lines():
        if line_hit(line):
            emit(line)

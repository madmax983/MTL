def min_running_balance(start, xs):
    balance = start
    lowest = start
    for delta in xs:
        balance += delta
        lowest = min(lowest, balance)
    return lowest

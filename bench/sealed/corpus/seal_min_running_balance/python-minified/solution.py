min_running_balance=lambda start,xs:min(__import__('itertools').accumulate(xs,initial=start))

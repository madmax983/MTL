digit_sum_base=lambda n,b:0 if n==0 else n%b+digit_sum_base(n//b,b)

collatz_steps=lambda n:0 if n==1 else 1+collatz_steps(n//2 if n%2==0 else 3*n+1)

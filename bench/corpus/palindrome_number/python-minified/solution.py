palindrome_number=lambda n,m=None,r=0:(1 if r==n else 0) if m==0 else palindrome_number(n,(n if m is None else m)//10,r*10+(n if m is None else m)%10)

#!/usr/bin/env python3
def counts(s):
    # tokcount encodes under o200k_base and cl100k_base
    import tiktoken
    o = len(tiktoken.get_encoding("o200k_base").encode(s))
    c = len(tiktoken.get_encoding("cl100k_base").encode(s))
    return o, c

pairs = {
 "sum_list":        ("[>0=][0][][+]|",                          "0[+]#"),
 "length_list":     ("[>0=][0][][~_1+]|",                       "0[_1+]#"),
 "product_list":    ("[>0=][1][][*]|",                          "1[*]#"),
 "max_list":        (">_[>[;0][[]1]?][_][>_[^^<[~_][_]?]'][]|",  ">_~[^^<[~_][_]?]#"),
 "min_list":        (">_[>[;0][[]1]?][_][>_[^^<[_][~_]?]'][]|",  ">_~[^^<[_][~_]?]#"),
 "reverse_list":    ("[]~[>[;0][[]1]?][_][>_[~;]'][]|",          "[][~;]#"),
 "contains":        ("0~@[>[;0][[]1]?][__][>_[^=@~+0~<~]'][]|",  "[=+0~<];0~#"),
 "count_occurrences":("0~@[>[;0][[]1]?][__][>_[^=@~+~]'][]|",    "[=+];0~#"),
}

order = ["max_list","min_list","reverse_list","contains","count_occurrences","sum_list","length_list","product_list"]

print(f"{'task':18} | {'before src':42} | bo | bc | {'after src':20} | ao | ac | do | dc")
print("-"*140)
sob=soc=soa=soac=0
for k in order:
    b,a = pairs[k]
    bo,bc = counts(b)
    ao,ac = counts(a)
    do = ao-bo; dc = ac-bc
    sob+=bo; soc+=bc; soa+=ao; soac+=ac
    print(f"{k:18} | {b:42} | {bo:2} | {bc:2} | {a:20} | {ao:2} | {ac:2} | {do:+d} | {dc:+d}")
print("-"*140)
print(f"{'TOTAL':18} | {'':42} | {sob:2} | {soc:2} | {'':20} | {soa:2} | {soac:2} | {soa-sob:+d} | {soac-soc:+d}")
print()
# 5 ugly only
print("=== 5 ugly accumulators only ===")
u=["max_list","min_list","reverse_list","contains","count_occurrences"]
b5o=b5c=a5o=a5c=0
for k in u:
    b,a=pairs[k]; bo,bc=counts(b); ao,ac=counts(a)
    b5o+=bo;b5c+=bc;a5o+=ao;a5c+=ac
print(f"before o200k={b5o} cl100k={b5c} ; after o200k={a5o} cl100k={a5c} ; delta o200k={a5o-b5o} cl100k={a5c-b5c}")

def digit_product(n):
    product = 1
    for digit in str(abs(n)):
        product *= int(digit)
    return product

# test: closures capture globals
base = 2
double = (x) -> x * base
double(21) == 42

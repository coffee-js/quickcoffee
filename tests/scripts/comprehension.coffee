# test: filtered comprehensions preserve source order
doubled = [value * 2 for value in [1..5] when value % 2 == 1]
doubled == [2, 6, 10]

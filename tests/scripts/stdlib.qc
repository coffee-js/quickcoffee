# test: the prototype-free standard library stays ordinary and composable
record = {a: 1, b: 2}
numbers = range(1, 4)
len(numbers) == 3 and type(record) == 'map' and str(numbers[2]) == '3' and join(keys(record), ',') == 'a,b' and join(values(record), ',') == '1,2' and split('a,b', ',')[1] == 'b' and assert(true) == nil

# test: the prototype-free standard library stays ordinary and composable
record = {a: 1, b: 2}
numbers = range(1, 4)
text = trim('\u{3000}coffee ☕\u{3000}')
len(numbers) == 3 and type(record) == 'map' and str(numbers[2]) == '3' and join(keys(record), ',') == 'a,b' and join(values(record), ',') == '1,2' and split('a,b', ',')[1] == 'b' and contains(text, '☕') and starts_with(text, 'coffee') and ends_with(text, '☕') and sort([3, 1, 2]) == [1, 2, 3] and abs(-3) == 3 and sum(numbers) == 6 and min(numbers) == 1 and max(numbers) == 3 and assert(true) == nil

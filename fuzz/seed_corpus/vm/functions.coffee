make_adder = (offset) ->
  (value) -> value + offset
add_two = make_adder(2)
sum = 0
for value, index in [1, 2, 3]
  sum += add_two(value) + index
sum

# test: loop control stays in bytecode and does not leak iterations
sum = 0
for value in [1..6] then if value == 3 then continue else if value == 6 then break else sum = sum + value
sum == 12

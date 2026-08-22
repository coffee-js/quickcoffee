## QuickCoffee manual
##
## Source is parsed, compiled to verified bytecode, and executed with a fuel budget.
## qcoffee - reads a QuickCoffee program from standard input.
## qcoffee --stats writes instruction and remaining-fuel counters to stderr while preserving program stdout.
## qcoffee --check FILE parses, compiles, and verifies without executing FILE.
## qtest --fuel N gives each executable documentation file its own instruction budget.
## Rust embedding errors expose ErrorKind::Parse, Verify, or Runtime plus a display-independent message; host callbacks may return Error::runtime("message"), and error.position() may give a one-based source line.
## Engine::compile_program and Context::run_program share verified bytecode for repeated embedding calls.
## Context::last_execution() exposes instruction and remaining-fuel counters without VM frames.
## Arguments after -- are exposed as the ordinary string array argv.
## This is not JavaScript: prototypes, this, eval, and embedded JavaScript do not exist.
## # is a line comment; ### … ### is a non-nesting block comment removed before layout and parsing.
## Identifiers use Unicode XID rules; combining marks may continue a name and no normalization occurs.
## yes/on and no/off are Boolean aliases; is/isnt preserve strict equality.
## Chained strict or numeric comparisons keep the middle value once and short-circuit.
## The standard library is ordinary functions: print, len, type, range, str, keys, values, join, split, and assert.
## switch/when selects one strict-equality branch without fallthrough.
## try/catch/finally handles QuickCoffee runtime errors without JavaScript Error objects.
## Integer ranges use [1..3] for an inclusive end and [1...3] for an exclusive end.
## Triple-quoted heredocs preserve newlines: """...""" interpolates and '''...''' remains literal.
## Array slices use a[start..end] for an inclusive end and a[start...end] for an exclusive end; bounds are finite in-range integers, negatives count from the end, and a nil-safe slice skips bounds on nil.
## Nil-specific fallback is written as left ? right; false and zero are kept unchanged.
## Postfix value? tests only non-nil: nil? is false, false? and 0? are true, and an unbound name remains an error.
## name ?= value writes only for an unbound or nil name; a non-nil name short-circuits its right side, and members, indexes, and patterns are excluded.
## value in array checks array membership; key of map checks only map-owned string keys.
## value not in array and key not of map negate those same strict checks without prototype keys.
## In a map literal, {name} abbreviates {name: name}.
## Assignment patterns may nest arrays and maps; validation is atomic before bindings change.
## In arrays and calls, items... expands an array without JavaScript apply.
## Nil-safe soak suffixes record?.name, values?[i], and fn?(args) short-circuit only a nil receiver.
## until condition then body repeats until its Boolean condition becomes true.
## At statement position, postfix while/until repeats a whole assignment or strict destructuring, not an ordinary subexpression.
## loop body is infinite while true; break exits it and fuel still bounds it.
## A for expression collects body values; when and continue omit values, and break keeps the collected prefix.
## for bindings may use strict patterns: for [left, right] in pairs binds each pair atomically.
## An array for loop may use by step; the positive finite integer step is evaluated once, and maps exclude it.
## Array for may bind a zero-based index too: for value, index in items then value + index.
## Postfix comprehensions use the same strict collector: value * 2 for value in items, or [value * 2 for value in items].
## Functions capture lexical scope; y = 2 defaults when omitted or nil, and a final rest parameter is tail....
## Plain names may omit lambda parentheses: left, right -> left + right; defaults, rest, and patterns retain parentheses.
## Names support strict arithmetic compound assignment: total += amount and power **= 2; members and indexes do not.
## Names also support strict prefix/postfix updates: next = ++counter yields the new value, while previous = counter-- yields the old value.
## Arithmetic also has floor division // and dividend-dependent modulo %%: -7 // 5 is -2, while -7 %% 5 is 3.
## return expression exits only its current function; bare return yields nil, cleans loops, and runs enclosing finally blocks.
## Parameters may use strict nested array/map patterns; defaults and rest stay name-only.
base = 20
add = (x) ->
  result = x + base
  result
shorthand = 'yes'
[first, {point: [x, y]}] = [0, {point: [20, 22]}]
scale = ([left, right], {factor}) -> (left + right) * factor
add(22) == 42 and "answer=#{add(22)}" == 'answer=42' and yes is on and no is off and 1 < 2 < 3 and x + y == 42 and scale([20, 1], {factor: 2}) == 42 and ((head, y = 2) -> head + y)(40) == 42 and ((head, tail...) -> head + len(tail))(40, 1, 2) == 42 and ((items) -> for n in items then if n == 42 then return n)([1, 42]) == 42 and ((-> try return 1 catch error then 2 finally 0)()) == 1 and len([1..3]) == 3 and len([1...3]) == 2 and (nil ? 42) == 42 and (false ? 42) == false and nil?.missing == nil and 2 in [1, 2] and 'name' of {name: 1} and {shorthand}.shorthand == 'yes' and len([1, [2, 3]..., 4]) == 4
try throw 'manual' catch error then error == 'runtime error: thrown: manual'
by_sum = 0
for n in [1..9] by 3 then by_sum = by_sum + n
by_sum == 12
len(for [left, right] in [[20, 22], [1, 2]] then left + right) == 2
postfix_doubles = value * 2 for value in [1..3]
postfix_doubles == [2, 4, 6]
counter = 2
prefix_update = ++counter
postfix_update = counter--
[prefix_update, postfix_update, counter] == [3, 3, 3]
[-7 // 5, -7 %% 5] == [-2, 3]
[5 & 3, 5 | 2, 5 ^ 1, ~1, 1 << 3, -8 >> 2, -1 >>> 1] == [1, 7, 4, -2, 8, -2, 2147483647]
continued = 1 +
  2 * 3
continued == 7
message = "hello
  world"
message == 'hello world'
folded = (1 + 2 * 3) == 7
folded
values = [
  1
  2
]
values == [1, 2]
record = {
  first: 20
  second: 22
}
record.first + record.second == 42
indented_record =
  first: 20
  nested:
    second: 22
indented_record.nested.second == 22
implicit_add = (left, right) -> left + right
implicit_answer = implicit_add 20, 22
implicit_answer == 42
3 not in [1, 2] and 'missing' not of {present: 1}
loop_count = 0
loop
  loop_count = loop_count + 1
  break if loop_count == 3
loop_count == 3
bare_add = left, right -> left + right
bare_add(20, 22) == 42
postfix_count = 0
postfix_count = postfix_count + 1 while postfix_count < 3
postfix_count == 3
slice_values = [0..4][1..3]
len(slice_values) == 3 and slice_values[0] == 1 and [0..4][-3...-1][0] == 2
nil? == false and false? == true and 0? == true
default_value ?= 42
default_value == 42
heredoc = """answer #{add(22)}
next"""
heredoc == 'answer 42\nnext'
### invalid ` source is safely ignored here
###
42 == 42

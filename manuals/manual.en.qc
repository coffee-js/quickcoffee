## QuickCoffee manual
##
## Source is parsed, compiled to verified bytecode, and executed with a fuel budget.
## qcoffee - reads a QuickCoffee program from standard input.
## qcoffee --quit initializes one Context and exits silently; it cannot be combined with source or execution options.
## qcoffee --stats writes instruction, remaining-fuel, hot-path, managed-value-allocation, and lexical-environment-allocation counters to stderr while preserving program stdout; qcoffee accepts one source input and rejects conflicting execution modes.
## Embedded modules may use named import/export; Engine::compile_module and Context::run_module obtain source only through a host ModuleLoader, keep module globals private, and share fuel across the graph.
## qcoffee --module-root ROOT ENTRY explicitly grants one restricted-file module root to that execution only; ENTRY is root-relative, imports stay ./ or ../, and success prints the ordered export Map (or an exports JSON record). Fuel, stats, and argv work there, while single-file, stdin, -e, REPL, check, disassembly, and fingerprint modes never gain that authority.
## Embedders may set Context ResourceLimits for general String UTF-8 bytes, Array items, Map entries, JSON sizes, collection-operation item counts, Integer bits, Decimal coefficient bits, and Decimal scale; constants, globals, native results, member reads, and generated values are rechecked under the current Context policy. Boundary failures are Resource errors that scripts cannot catch, while JSON syntax errors remain catchable.
## IntoValue and TryFromValue convert owned host scalars, Vec, BTreeMap<String, T>, and Option recursively without running a script; nil alone maps to None, and Number, Integer, Decimal, and other kinds never coerce across their boundaries.
## qcoffee --check FILE parses, compiles, and verifies without executing FILE.
## qcoffee --interactive (or -i) keeps one Context for a line-oriented session; :help and :quit are built-in commands.
## qcoffee --interactive --stats writes one instruction/fuel record for each non-empty line that executes or reaches a runtime error; parse and verify errors write none.
## for character, index in 'a☕中' then index yields [0, 1, 2]; strings iterate Unicode scalars and accept non-zero signed by steps.
## do (name, other) -> ... immediately calls and forwards same-named outer values; do -> ... remains zero-argument.
## [head, tail...] = [1, 2, 3] binds tail to [2, 3]; array-pattern rest must be final.
## qtest --fuel N gives each executable documentation file its own instruction budget.
## qtest --stats writes each file's instruction count and remaining fuel to stderr without changing its ok output.
## qtest --json writes one stable JSON result per file for CI consumers; --stats remains on stderr.
## qtest --tap writes TAP version 13 records with deterministic numbering; --json and --tap are mutually exclusive.
## qtest --filter TEXT selects matching paths, while qtest --list enumerates selected files without executing them.

## qcoffee --json emits one JSON value or structured error for a single execution, suitable for CI and hosts.
## Rust embedding errors expose ErrorKind::Parse, Verify, Runtime, or Resource plus a display-independent message; error.resource_limit() distinguishes fuel, call depth, cancellation, JSON boundaries, StringBytes, ArrayItems, MapEntries, IntegerBits, DecimalCoefficientBits, DecimalScale, and CollectionOperationItems. Host callbacks may return Error::runtime("message"), and error.position() may give a one-based source line.
## Engine::compile_program verifies once; Context::run_program reuses the immutable verified bytecode for repeated embedding calls.
## Program::fingerprint provides a deterministic u64 bytecode cache key without changing execution.
## qcoffee --fingerprint FILE prints the same verified bytecode key as 16 lowercase hexadecimal digits without running the file.
## qbench --json emits one timing record per guarded workload; --iterations controls sample count.
## Each qbench record's profile_* fields come from one untimed execution and report hot paths and allocation events without scaling by --iterations or --repeat.
## qbench --compare-qjs PATH separates startup, compilation, precompiled hot execution, and end-to-end CLI time for both runtimes. Reports should use --repeat 11; each phase has a median and *_mad_ns.
## Fingerprints use explicit canonical bytecode encoding, not Rust debug formatting, so cache keys survive toolchain display changes.
## qdocco --markdown writes Notes, fenced QuickCoffee code, and the final value as a reviewable Markdown artifact.
## Embedders may call Context::set_fuel or set_resource_limits between runs without clearing globals; with_resource_limits, with_global, and with_native provide chainable setup.
## cargo run --example embed compiles a minimal Rust host that sets a global, registers a native callback, and evaluates QuickCoffee.
## A host can branch on Value::kind() and use Value::is_nil() without inspecting internal containers.
## Cargo package metadata points embedding users to the repository, docs.rs API, README, and license.
## Context::last_execution() exposes instruction and remaining-fuel counters without VM frames.
## Arguments after -- are exposed as the ordinary string array argv.
## This is not JavaScript: public prototypes, global/free this, eval, and embedded JavaScript do not exist. Indented classes support constructors, instance/static methods, confined receivers, new, private extends chains, statically resolved super, and receiver-bound => methods or nested closures that can escape safely.
class BoundCounter
  constructor: (@value) ->
  callback: ->
    =>
      @value = @value + 1
      @value

bound_callback = new BoundCounter(40).callback()
bound_callback()
## # is a line comment; ### … ### is a non-nesting block comment removed before layout and parsing.
## Identifiers use Unicode XID rules; combining marks may continue a name and no normalization occurs.
## yes/on and no/off are Boolean aliases; is/isnt preserve strict equality.
## ! is a strict Bool alias for not; != remains strict inequality.
## Chained strict or numeric comparisons keep the middle value once and short-circuit.
## The standard library is ordinary functions: print, len, type, error, range, str, trim, contains, starts_with, ends_with, sort, parse_json, encode_json, integer, number, decimal, decimal_div, round_decimal, abs, sum, min, max, keys, values, join, split, and assert. RFC 0139 string queries are strict and locale-free; trim uses a pinned Unicode White_Space table. RFC 0140 sort returns a new stable array of homogeneous finite scalar values and uses locale-free Unicode-scalar String order. error(code, message, data, cause) creates a sealed RFC 0136 Error; catch binds Error and resource failures remain uncatchable. Aggregators accept homogeneous finite Number, Integer, or Decimal arrays.
trimmed_text = trim('\u{3000}coffee ☕\u{3000}')
contains(trimmed_text, '☕') and starts_with(trimmed_text, 'coffee') and ends_with(trimmed_text, '☕')
sort(['中', 'a', '☕']) == ['a', '☕', '中']
## RFC 0137 Decimal literals use an m suffix; exact division rejects repeating results, while decimal_div and round_decimal require an explicit scale and rounding mode.
## switch/when selects one strict-equality branch without fallthrough.
## try/catch/finally handles sealed QuickCoffee Error values without JavaScript prototypes, stacks, or forgeable source locations.
## Integer ranges use [1..3] for an inclusive end and [1...3] for an exclusive end.
## Ranges may descend too: [3..1] yields [3, 2, 1], while [3...1] yields [3, 2].
## Triple-quoted heredocs preserve newlines: """...""" interpolates and '''...''' remains literal.
## Array slices use a[start..end] for an inclusive end and a[start...end] for an exclusive end; bounds are finite in-range integers, negatives count from the end, and a nil-safe slice skips bounds on nil.
## Nil-specific fallback is written as left ? right; false and zero are kept unchanged.
## Postfix value? tests only non-nil: nil? is false, false? and 0? are true, and an unbound name remains an error.
## name ?= value writes only for an unbound or nil name; a non-nil name short-circuits its right side, and members, indexes, and patterns are excluded.
## value in array checks array membership; key of map checks only map-owned string keys.
## value not in array and key not of map negate those same strict checks without prototype keys.
## In a map literal, {name} abbreviates {name: name}.
## Map literals support checked left-to-right spread: {...defaults, theme: 'dark'}; later keys win.
## Map patterns may end with ...metadata to capture unlisted keys immutably.
## Arrays and Unicode strings accept negative indices: items[-1] is the final item.
## Assignment patterns may nest arrays and maps; validation is atomic before bindings change.
## In arrays and calls, items... expands an array without JavaScript apply.
## Nil-safe soak suffixes record?.name, values?[i], and fn?(args) short-circuit only a nil receiver.
## until condition then body repeats until its Boolean condition becomes true.
## At statement position, postfix while/until repeats a whole assignment or strict destructuring, not an ordinary subexpression.
## loop body is infinite while true; break exits it and fuel still bounds it.
## A for expression collects body values; when and continue omit values, and break keeps the collected prefix.
## for bindings may use strict patterns: for [left, right] in pairs binds each pair atomically.
## An array for loop may use by step; the non-zero finite integer step is evaluated once, negative steps start at the last item, and maps exclude it.
## Array for may bind a zero-based index too: for value, index in items then value + index.
## Postfix comprehensions use the same strict collector: value * 2 for value in items, or [value * 2 for value in items].
## Functions capture lexical scope; y = 2 defaults when omitted or nil, and a final rest parameter is tail....
## Plain names may omit lambda parentheses: left, right -> left + right; defaults, rest, and patterns retain parentheses.
## Names support strict arithmetic compound assignment: total += amount and power **= 2; members and indexes do not.
## Names also support strict prefix/postfix updates: next = ++counter yields the new value, while previous = counter-- yields the old value.
## Arithmetic also has floor division // and dividend-dependent modulo %%: -7 // 5 is -2, while -7 %% 5 is 3.
## return expression exits only its current function; bare return yields nil, cleans loops, and runs enclosing finally blocks.
## Parameters may use strict nested array/map patterns; defaults and rest stay name-only.
class ManualPoint
  constructor: (@x, @y = 2) ->
  sum: -> @x + @y
  @origin: -> new ManualPoint(0, 0)
class NamedManualPoint extends ManualPoint
  constructor: (x, y) -> super(x, y)
  sum: -> super() + 1
manual_point = new ManualPoint(40)
named_manual_point = new NamedManualPoint(39, 2)
manual_point.sum() == 42 and ManualPoint.origin().sum() == 0 and named_manual_point.sum() == 42 and type(manual_point) == 'instance'
base = 20
add = (x) ->
  result = x + base
  result
shorthand = 'yes'
[first, {point: [x, y]}] = [0, {point: [20, 22]}]
scale = ([left, right], {factor}) -> (left + right) * factor
add(22) == 42 and "answer=#{add(22)}" == 'answer=42' and yes is on and no is off and 1 < 2 < 3 and x + y == 42 and scale([20, 1], {factor: 2}) == 42 and ((head, y = 2) -> head + y)(40) == 42 and ((head, tail...) -> head + len(tail))(40, 1, 2) == 42 and ((items) -> for n in items then if n == 42 then return n)([1, 42]) == 42 and ((-> try return 1 catch error then 2 finally 0)()) == 1 and len([1..3]) == 3 and len([1...3]) == 2 and (nil ? 42) == 42 and (false ? 42) == false and nil?.missing == nil and 2 in [1, 2] and 'name' of {name: 1} and {shorthand}.shorthand == 'yes' and len([1, [2, 3]..., 4]) == 4
try throw 'manual' catch problem then problem.code == 'throw' and problem.data == 'manual'
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
escaped = "A\\x42\\u{43}"
escaped == 'ABC'
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
0.1m + 0.2m == 0.3m and decimal_div(1m, 3m, 2, 'half_even') == 0.33m
json_payload = parse_json('{"money":12.30,"large":9007199254740993}')
encode_json(json_payload) == '{"large":9007199254740993,"money":12.3}'
42 == 42

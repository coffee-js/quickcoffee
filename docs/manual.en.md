# QuickCoffee manual

QuickCoffee is a Rust bytecode engine, not a JavaScript runtime. Source is parsed, compiled, verified, then run. There are no prototypes, `this`, `eval`, or embedded JavaScript.

Triple-quoted heredocs preserve newlines: `"""…"""` interpolates `#{expression}`, while `'''…'''` stays literal. Their content is not indentation-trimmed; an unclosed delimiter is a lexical error.

`#` starts a line comment. A non-nesting `### … ###` block comment is removed before layout and parsing; an unclosed delimiter is a lexical error.

Identifiers use Unicode XID rules: XID start or `_` first, then XID continue or `_`. Combining marks may therefore continue a name; the engine does not normalize Unicode.

Quoted strings decode common control escapes plus `\\xNN`, `\\uNNNN`, and `\\u{...}` Unicode escapes. Invalid escapes and non-scalar Unicode values are parse errors.

CoffeeScript-style spellings are available without changing runtime types: `yes`/`on` mean `true`, `no`/`off` mean `false`, and `is`/`isnt` mean strict `==`/`!=`.

Adjacent strict or numeric comparisons may chain: `1 < middle() < 3` evaluates `middle()` once and stops before later operands when an earlier comparison is false.

Run `qcoffee -e "print(range(1, 4))"`, `qcoffee --fuel 10000 program.qc`, `qcoffee --check program.qc`, or `qcoffee --dump-bytecode program.qc`. `--check` parses, compiles, and verifies without executing. Fuel limits executed instructions; exhaustion is a safe error. The small standard library contains `print`, `len`, `type`, end-exclusive `range(a, b)`, `str`, `keys`, `values`, `join`, `split`, and `assert`.

`qcoffee -` reads source from standard input, which is convenient in pipelines; `qcoffee --dump-bytecode -` disassembles that same input instead of executing it.
`qcoffee --stats` writes instruction and remaining-fuel counters to stderr while preserving program stdout; it cannot be combined with `--check` or `--dump-bytecode`.

`qcoffee --interactive` (or `-i`) keeps one Context across input lines; `:help` lists commands and `:quit`/`:exit` leave the session. Piped input receives no prompts.
With `--stats`, each non-empty interactive line that executes or reaches a runtime error writes its instruction and remaining-fuel counters to stderr; parse and verify errors write no fresh record.
`for character, index in 'a☕中' then index` yields `[0, 1, 2]`; strings iterate Unicode scalars and reject `by`.
[head, tail...] = [1, 2, 3] binds tail to [2, 3]; array-pattern rest must be final.

Arguments after `--` are exposed as the ordinary string array `argv`: `qcoffee program.qc -- first second` makes `len(argv)` evaluate to `2`. No host process or environment object is exposed.

Functions use `(x) -> expression` or bare names such as `left, right -> left + right`, and capture their lexical environment. Defaults, rest, and patterns still require parentheses. Calls may omit parentheses on one logical line, while explicit parentheses remain available for unambiguous grouping. A trailing parameter may have a default, as in `(head, separator = '-') -> expression`; an omitted or explicit `nil` argument evaluates that default inside the callee, so it may use earlier parameters and lexical captures. Required parameters must precede defaults. A final rest parameter (`(head, tail...) -> expression`) accepts remaining values, bound as an array. Maps are indexed by strings: `{name: 'coffee'}['name']`.

`return expression` is valid only in a function and immediately exits that function; bare `return` yields `nil`. It never crosses a nested function. It cleans an active loop and runs enclosing `finally` blocks from inner to outer; a `return` in `finally` replaces the pending result. Write `if condition then return value` for a conditional return.

Parameters may also use strict recursive patterns: `([left, right], {factor}) -> (left + right) * factor`. Each argument must match its pattern before the function starts. Defaults remain name-only and rest remains a final name.

Within a map literal, `{name}` is shorthand for `{name: name}`; string keys still require an explicit value, as in `{'name': value}`.

Assignment patterns can nest arrays and maps: `[first, {point: [x, y]}] = [1, {point: [20, 22]}]`. Arrays match their exact length; maps require their listed identifier keys. The VM validates the full pattern before changing any binding, so a deep mismatch is atomic.

An array or call item with trailing `...` expands an array: `[1, values..., 4]` concatenates its elements, while `fn(values...)` passes them as individual arguments. A splat must be an array; it never invokes a JavaScript-style `apply` method.

Nil-safe suffixes use CoffeeScript-style soak syntax: `record?.name`, `values?[index]`, and `fn?(args)`. If the receiver is `nil`, the suffix returns `nil` and does not evaluate an index or argument. A non-nil receiver follows ordinary strict access rules, so a missing map member still reports an error.

Arrays (including `range` results) can be iterated with `for item in items then expression`, or with one-time-evaluated positive-integer stepping: `for item in [1..9] by 3 then expression`. A second binding receives the zero-based position, as in `for item, index in items then item + index`; with `by`, it receives the actual stepped position. The binding is a strict recursive pattern, so `for [left, right] in pairs then left + right` and `for {point: {x, y}} in values then x + y` are valid; all bindings for an item change atomically. A `for` expression collects body values into a new array; rejected `when` items are omitted and `break` returns the collected prefix. `break` and `continue` affect the innermost loop. `while`/`until`/`loop` evaluate to `nil`; map iteration excludes `by`.

The same collector has CoffeeScript's postfix comprehension form: `value * 2 for value in items`, or `[value * 2 for value in items]`. The brackets delimit the comprehension and do not create an extra nested array; `by`, `when`, map iteration, patterns, `break`, and `continue` retain their prefix-form semantics.

Integer range literals are arrays built directly by the bytecode VM: `[1..3]` includes its end (`[1, 2, 3]`), while `[1...3]` excludes it (`[1, 2]`); descending forms work too, so `[3..1]` is `[3, 2, 1]`. Their bounds must be finite integers.

Array slices use `items[start..end]` for an inclusive end and `items[start...end]` for an exclusive end: `[0..4][1..3]` is `[1, 2, 3]`. Bounds evaluate once from left to right and must be finite in-range integers; negative bounds count from the end, so `-1` is the last item. Slices are arrays only and never clip implicitly. `items?[start..end]` returns `nil` without evaluating bounds when its receiver is `nil`.

`left ? right` is a nil-specific fallback: it evaluates `right` only when `left` is `nil`. Unlike a truthiness default, it preserves `false`, `0`, empty strings, and empty containers.

The postfix `value?` tests only for non-nil: `nil?` is `false`, while `false?` and `0?` are `true`. It does not hide an unbound-name error and is distinct from the `left ? right` fallback.

`name ?= value` evaluates and stores `value` only when the name is unbound or currently nil; a non-nil value skips the right side. Names also support strict arithmetic compound assignment such as `total += amount` and `power **= 2`. These forms are name-only, never member/index/destructuring assignment, and an ordinary unbound-name read remains an error.

Names also support strict numeric updates: `next = ++counter` yields the new value, while `previous = counter--` yields the old value before decrementing. Updates are name-only and reject members, indexes, and destructuring.

CoffeeScript arithmetic also provides floor division `a // b` and dividend-dependent modulo `a %% b`; for example, `-7 // 5` is `-2` and `-7 %% 5` is `3`. Ordinary `%` remains the signed remainder.

Bitwise operators use strict signed 32-bit numbers: `&`, `|`, `^`, `~`, `<<`, `>>`, and `>>>`; shifts accept counts from 0 through 31, and compound forms are name-only.

An explicit operator at a physical line end continues the expression on the next line; continuation indentation is layout-neutral until the expression ends.

Ordinary quoted strings may span lines: a newline joins as one space, while a trailing backslash removes the newline.

Pure literal arithmetic such as `(1 + 2 * 3) == 7` is folded into verified bytecode constants.

`value in array` checks array membership with QuickCoffee equality, while `value not in array` negates it. `key of map` checks an own string key in a map, while `key not of map` negates it; maps have no prototype keys to inspect.

`until condition then body` is the inverse loop form: it repeats until its Boolean condition becomes true, using the same `break`, `continue`, indentation, and fuel rules as `while`.

At statement position, `n = n + 1 while n < 3` is a postfix loop equivalent to the prefix while and repeats the whole assignment; `until` works likewise. Strict destructuring may also be its body. A postfix loop cannot be nested inside an ordinary subexpression.

`loop body` is the infinite `while true` form. Exit it with `break`; it remains fuel-limited. For example: `n = 0; loop then if n == 3 then break else n = n + 1`.

Put `when condition` between a `for` iterable and `then` to filter a loop without running the body for rejected bindings: `for n in [1..5] when n > 2 then print(n)`.

A prototype-free data factory uses `class Point(x, y = 0) -> {x: x, y: y}` and follows the same default-parameter rules as a function. Calling it returns an ordinary map, so `Point(3).x` reads a member; there is no `this`, `new`, or inheritance.

Double quotes interpolate QuickCoffee expressions: `"answer=#{add(21)}"`. Single quotes do not, and interpolation never runs JavaScript.

Use `switch value` with indented `when pattern` branches for strict-equality selection. Exactly one branch is selected; there is no fallthrough.

Exceptions use `try`, `catch error`, optional `finally`, and `throw value`. A catch receives a stable error string rather than a JavaScript Error object; function returns also run applicable finalizers.

For Rust embedding, create `Context`, optionally call `with_fuel`, register a host callback with `add_native`, then call `eval`; callbacks can return `Error::runtime("message")` and the script may catch it. For repeated execution, compile once with `Engine::compile_program` (which verifies once) and pass the shared `Program` to `run_program`; cloning that handle does not copy bytecode or repeat verification. `Value::from`, `Value::string`, `Value::array`, and `Value::map` construct host values without exposing VM reference-counting internals.

`Context::last_execution()` returns public `ExecutionStats` (`instructions` and `fuel_remaining`) for the latest successful or runtime-failed execution; compile and verification errors leave the previous record unchanged.

`cx.get_global("host_values")` reads a script or host global without executing code and returns `None` for an unknown name. It returns a public `Value` clone only, never an environment or call frame.

Embedding errors are structured: `error.kind()` returns `ErrorKind::Parse`, `ErrorKind::Verify`, or `ErrorKind::Runtime`, `error.message()` returns its detail, and `error.position()` may provide a one-based source line. Hosts need not parse display text; `Display` remains suitable for CLI output and QuickCoffee `catch` strings.

```rust
let mut cx = quickcoffee::Context::new().with_fuel(100_000);
cx.set_global(
    "host_values",
    quickcoffee::Value::array(vec![
        quickcoffee::Value::from(40_i64),
        quickcoffee::Value::from(2_i64),
    ]),
);
let value = cx.eval("host_values[0] + host_values[1]")?;
```

`qdocco FILE -o FILE.html` verifies and renders executable documentation; `qtest FILE_OR_DIRECTORY...` recursively discovers `.qc` files and passes only when every final value is `true`.

`qtest --fuel N FILE_OR_DIRECTORY...` gives every discovered test file its own instruction budget, so a deliberately bounded loop cannot consume the budget of another test.
`qtest --stats` additionally writes each file's instruction count and remaining fuel to stderr without changing its `ok` output.

Multiline arrays and maps may omit commas at line boundaries; calls and ordinary parentheses still require explicit separators.

An indented map may follow a standalone assignment (`record =`); nested `key: value` entries become a prototype-free map without changing ordinary assignment continuations.

Calls may omit parentheses on one logical line: `implicit_answer = implicit_add 20, 22`; explicit parentheses remain available for unambiguous comparisons and layout boundaries.

`qtest --json` emits one stable JSON record per test file, while `qtest --tap` emits TAP 13 records. `qcoffee --fingerprint FILE` prints a canonical verified-bytecode cache key without executing the file. `qbench --json` reports guarded compile, verify, and execute timings; `qdocco --markdown` writes reviewable literate Markdown. Embedding hosts can adjust a reused context with `Context::set_fuel` and run the complete host example with `cargo run --example embed`.

//! Run with `cargo bench --bench core`. This uses no external benchmark framework.
use quickcoffee::{Context, Engine};
use std::{hint::black_box, time::Instant};

struct Workload {
    name: &'static str,
    source: &'static str,
    iterations: u32,
    expected: &'static str,
}

fn main() {
    let workloads = [
        Workload {
            name: "loop-core",
            source: "sum = 0\ni = 0\nwhile i < 100 then i = i + 1\nsum + i",
            iterations: 20_000,
            expected: "100",
        },
        Workload {
            name: "postfix-loops",
            source: "sum = 0\ni = 0\ni = i + 1 while i < 100\nsum + i",
            iterations: 20_000,
            expected: "100",
        },
        Workload {
            name: "array-slices",
            source: "items = [0...100]\nsum = 0\ni = 0\nwhile i < 100\n  slice = items[10...90]\n  sum = sum + slice[0] + slice[79]\n  i = i + 1\nsum",
            iterations: 10_000,
            expected: "9900",
        },
        Workload {
            name: "existence-tests",
            source: "value = nil\nsum = 0\ni = 0\nwhile i < 100\n  sum = sum + (if value? then 0 else 1)\n  i = i + 1\nsum",
            iterations: 10_000,
            expected: "100",
        },
        Workload {
            name: "existential-assignment",
            source: "value = 1\nsum = 0\ni = 0\nwhile i < 100\n  value ?= 2\n  sum = sum + value\n  i = i + 1\nsum",
            iterations: 10_000,
            expected: "100",
        },
        Workload {
            name: "name-updates",
            source: "i = 0\nsum = 0\nwhile i < 100\n  sum += i\n  i++\nsum",
            iterations: 10_000,
            expected: "4950",
        },
        Workload {
            name: "floor-modulo",
            source: "sum = 0\ni = -100\nwhile i < 100\n  sum += i // 3\n  sum += i %% 7\n  i += 1\nsum",
            iterations: 10_000,
            expected: "500",
        },
        Workload {
            name: "bitwise",
            source: "sum = 0\ni = -100\nwhile i < 100\n  sum += (i & 31) ^ (i << 1)\n  i += 1\nsum",
            iterations: 10_000,
            expected: "-196",
        },
        Workload {
            name: "multiline-strings",
            source: "message = \"alpha\n  beta\"\nlen(message)",
            iterations: 10_000,
            expected: "10",
        },
        Workload {
            name: "string-escapes",
            source: "message = \"A\\x42\\u{43}\"\nlen(message) + (if message == 'ABC' then 1 else 0)",
            iterations: 20_000,
            expected: "4",
        },
        Workload {
            name: "multiline-collections",
            source: "values = [\n  1\n  2\n  3\n]\nrecord = {\n  first: 1\n  second: 2\n}\nvalues[2] + record.first + record.second",
            iterations: 10_000,
            expected: "6",
        },
        Workload {
            name: "indented-maps",
            source: "record =\n  first: 1\n  nested:\n    second: 2\nrecord.nested.second + record.first",
            iterations: 10_000,
            expected: "3",
        },
        Workload {
            name: "implicit-calls",
            source: "add = (left, right) -> left + right\nanswer = add 20, 22\nanswer",
            iterations: 20_000,
            expected: "42",
        },
        Workload {
            name: "execution-stats",
            source: "sum = 0\ni = 0\nwhile i < 100\n  sum += i\n  i++\nsum",
            iterations: 10_000,
            expected: "4950",
        },
        Workload {
            name: "constant-folding",
            source: "value = (1 + 2 * 3) == 7\nvalue",
            iterations: 20_000,
            expected: "true",
        },
        Workload {
            name: "closures-and-ranges",
            source: "base = 1\nadd = (n) -> n + base\nsum = 0\nfor n in [1...50] then sum = sum + add(n)\nsum",
            iterations: 10_000,
            expected: "1274",
        },
        Workload {
            name: "bare-lambda",
            source: "base = 1\nadd = n -> n + base\nsum = 0\nfor n in [1...50] then sum = sum + add(n)\nsum",
            iterations: 10_000,
            expected: "1274",
        },
        Workload {
            name: "stepped-iteration",
            source: "sum = 0\nfor n in [1...100] by 3 then sum = sum + n\nsum",
            iterations: 10_000,
            expected: "1617",
        },
        Workload {
            name: "for-collection",
            source: "values = for n in [1...100] when n % 3 == 0 then n * 2\nlen(values)",
            iterations: 10_000,
            expected: "33",
        },
        Workload {
            name: "postfix-comprehension",
            source: "values = n * 2 for n in [1...100]\nsum = 0\nfor n in values then sum = sum + n\nsum",
            iterations: 10_000,
            expected: "9900",
        },
        Workload {
            name: "for-pattern-bindings",
            source: "pairs = for n in [1...100] then [n, n + 1]\nsum = 0\nfor [left, right] in pairs then sum = sum + left + right\nsum",
            iterations: 10_000,
            expected: "9999",
        },
        Workload {
            name: "maps-and-control",
            source: "record = {a: 1, b: 2, c: 3}\nsum = 0\nfor own key, value of record when value > 1 then sum = sum + value\ntry sum ? 0 catch error then 0",
            iterations: 10_000,
            expected: "5",
        },
        Workload {
            name: "soak-access",
            source: "record = {answer: 1}\nnone = nil\nsum = 0\ni = 0\nwhile i < 100\n  sum = sum + record?.answer + (none?[i] ? 0)\n  i = i + 1\nsum",
            iterations: 10_000,
            expected: "100",
        },
        Workload {
            name: "nested-destructuring",
            source: "sum = 0\ni = 0\nwhile i < 100\n  [first, {point: [x, y]}] = [1, {point: [2, 3]}]\n  sum = sum + first + x + y\n  i = i + 1\nsum",
            iterations: 10_000,
            expected: "600",
        },
        Workload {
            name: "chained-comparisons",
            source: "low = 0\nmiddle = 1\nhigh = 2\nsum = 0\ni = 0\nwhile i < 100\n  sum = sum + (if low < middle < high then 1 else 0)\n  i = i + 1\nsum",
            iterations: 10_000,
            expected: "100",
        },
        Workload {
            name: "destructuring-parameters",
            source: "scale = ([left, right], {factor}) -> (left + right) * factor\nsum = 0\ni = 0\nwhile i < 100\n  sum = sum + scale([1, 2], {factor: 3})\n  i = i + 1\nsum",
            iterations: 10_000,
            expected: "900",
        },
        Workload {
            name: "return-cleanup",
            source: "find = (items) ->\n  try\n    for n in items then if n == 73 then return n\n    nil\n  catch error\n    0\n  finally\n    0\nsum = 0\ni = 0\nwhile i < 100\n  sum = sum + find([1...100])\n  i = i + 1\nsum",
            iterations: 10_000,
            expected: "7300",
        },
    ];
    let engine = Engine::new();
    for workload in workloads {
        measure(&engine, workload);
    }
}

fn measure(engine: &Engine, workload: Workload) {
    let start = Instant::now();
    for _ in 0..workload.iterations {
        black_box(
            engine
                .compile(black_box(workload.source))
                .expect("benchmark source compiles"),
        );
    }
    let compile = start.elapsed();
    let program = engine
        .compile_program(workload.source)
        .expect("benchmark program compiles");
    let start = Instant::now();
    for _ in 0..workload.iterations {
        let verified = program.verify().is_ok();
        assert!(black_box(verified), "benchmark bytecode verifies");
    }
    let verify = start.elapsed();
    let mut semantic_context = Context::new().with_fuel(100_000);
    let observed = semantic_context
        .run_program(&program)
        .expect("benchmark semantic check runs");
    assert_eq!(
        observed.to_string(),
        workload.expected,
        "benchmark {} returned an unexpected value",
        workload.name
    );
    assert!(semantic_context.last_execution().instructions > 0);
    let start = Instant::now();
    for _ in 0..workload.iterations {
        black_box(
            Context::new()
                .with_fuel(100_000)
                .run_program(&program)
                .expect("benchmark chunk runs"),
        );
    }
    let execute = start.elapsed();
    println!("{} ({} iterations)", workload.name, workload.iterations);
    println!(
        "  compile: {:.3}ms total, {:.0} programs/s",
        compile.as_secs_f64() * 1_000.,
        workload.iterations as f64 / compile.as_secs_f64()
    );
    println!(
        "  verify: {:.3}ms total, {:.0} programs/s",
        verify.as_secs_f64() * 1_000.,
        workload.iterations as f64 / verify.as_secs_f64()
    );
    println!(
        "  execute: {:.3}ms total, {:.0} programs/s",
        execute.as_secs_f64() * 1_000.,
        workload.iterations as f64 / execute.as_secs_f64()
    );
}

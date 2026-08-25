//! Release benchmark runner with semantic guards and machine-readable timing output.

use quickcoffee::{Context, Engine};
use std::{
    env,
    process::{Command, ExitCode},
    time::Instant,
};

const OUTPUT_SCHEMA: &str = "quickcoffee.qbench.v1";
const COMPARISON_SCHEMA: &str = "quickcoffee.qcompare.v1";

struct Workload {
    name: &'static str,
    source: &'static str,
    expected: &'static str,
}
struct ComparisonWorkload {
    name: &'static str,
    quickcoffee: &'static str,
    quickjs: &'static str,
    expected: &'static str,
}

const COMPARISON_WORKLOADS: &[ComparisonWorkload] = &[
    ComparisonWorkload {
        name: "scalar-loop",
        quickcoffee: "sum = 0\ni = 0\nwhile i < 1000000\n  sum += i\n  i++\nsum",
        quickjs: "(function () { let sum = 0; for (let i = 0; i < 1000000; i++) sum += i; return sum; })",
        expected: "499999500000",
    },
    ComparisonWorkload {
        name: "function-loop",
        quickcoffee: "increment = (value) -> value + 1\nsum = 0\ni = 0\nwhile i < 250000\n  sum = increment(sum)\n  i++\nsum",
        quickjs: "(function () { const increment = value => value + 1; let sum = 0; for (let i = 0; i < 250000; i++) sum = increment(sum); return sum; })",
        expected: "250000",
    },
    ComparisonWorkload {
        name: "array-build-index-iterate",
        quickcoffee: "values = [0...1000]\nsum = 0\nindex = 0\nwhile index < 1000\n  sum += values[index] * 3\n  index++\nsum",
        quickjs: "(function () { /* array-build-index-iterate */ const values = Array.from({length: 1000}, (_, n) => n); let sum = 0; let index = 0; while (index < 1000) { sum += values[index] * 3; index++; } return sum; })",
        expected: "1498500",
    },
    ComparisonWorkload {
        name: "map-own-lookup",
        quickcoffee: "record = {alpha: 1, beta: 2, gamma: 3, delta: 4}\nsum = 0\ni = 0\nwhile i < 10000\n  sum += record.alpha + record.beta + record.gamma + record.delta\n  i++\nsum",
        quickjs: "(function () { /* map-own-lookup */ const record = {alpha: 1, beta: 2, gamma: 3, delta: 4}; let sum = 0; for (let i = 0; i < 10000; i++) sum += record.alpha + record.beta + record.gamma + record.delta; return sum; })",
        expected: "100000",
    },
    ComparisonWorkload {
        name: "map-functional-update",
        quickcoffee: "record = {alpha: 1, beta: 2, gamma: 3}\nsum = 0\ni = 0\nwhile i < 1000\n  record = {...record, beta: record.beta + 1}\n  sum += record.alpha + record.beta + record.gamma\n  i++\nsum + record.beta",
        quickjs: "(function () { /* map-functional-update */ let record = {alpha: 1, beta: 2, gamma: 3}; let sum = 0; for (let i = 0; i < 1000; i++) { record = {...record, beta: record.beta + 1}; sum += record.alpha + record.beta + record.gamma; } return sum + record.beta; })",
        expected: "507502",
    },
    ComparisonWorkload {
        name: "unicode-scalar-iterate",
        quickcoffee: "text = 'a☕中🙂z'\nsum = 0\nround = 0\nwhile round < 1000\n  for character, index in text\n    sum += index + len(character)\n  round++\nsum",
        quickjs: "(function () { /* unicode-scalar-iterate */ const text = 'a☕中🙂z'; let sum = 0; for (let round = 0; round < 1000; round++) { let index = 0; for (const character of text) { sum += index + Array.from(character).length; index++; } } return sum; })",
        expected: "15000",
    },
    ComparisonWorkload {
        name: "unicode-scalar-index",
        quickcoffee: "text = 'a☕中🙂z'\nsum = 0\ni = 0\nwhile i < 10000\n  character = text[i % 5]\n  sum += if character == '🙂' then 7 else len(character)\n  i++\nsum",
        quickjs: "(function () { /* unicode-scalar-index */ const scalars = Array.from('a☕中🙂z'); let sum = 0; for (let i = 0; i < 10000; i++) { const character = scalars[i % 5]; sum += character === '🙂' ? 7 : Array.from(character).length; } return sum; })",
        expected: "22000",
    },
];

const WORKLOADS: &[Workload] = &[
    Workload {
        name: "loop-core",
        source: "sum = 0\ni = 0\nwhile i < 100 then i = i + 1\nsum + i",
        expected: "100",
    },
    Workload {
        name: "stdlib-abs",
        source: "abs(-42)",
        expected: "42",
    },
    Workload {
        name: "stdlib-sum",
        source: "sum([1, 2, 3, 4])",
        expected: "10",
    },
    Workload {
        name: "stdlib-min-max",
        source: "min([3, 1, 2]) + max([3, 1, 2])",
        expected: "4",
    },
    Workload {
        name: "stdlib-range-sum",
        source: "sum(range(1, 100))",
        expected: "4950",
    },
    Workload {
        name: "closures-and-ranges",
        source: "base = 1\nadd = (n) -> n + base\nsum = 0\nfor n in [1...50] then sum = sum + add(n)\nsum",
        expected: "1274",
    },
    Workload {
        name: "call-containing-local-loop",
        source: "increment = (value) -> value + 1\nrun = (limit) ->\n  sum = 0\n  i = 0\n  while i < limit\n    sum = increment(sum)\n    i++\n  sum\nrun(50)",
        expected: "50",
    },
    Workload {
        name: "captured-local-loop",
        source: "make_counter = (limit) ->\n  value = 0\n  read = -> value\n  i = 0\n  while i < limit\n    value++\n    i++\n  read\ncounter = make_counter(50)\ncounter()",
        expected: "50",
    },
    Workload {
        name: "map-spread",
        source: "base = {a: 1, b: 2}\nout = {...base, b: 3, c: 4}\nout.a + out.b + out.c",
        expected: "8",
    },
    Workload {
        name: "member-lookup-loop",
        source: "record = {alpha: 1, beta: 2, gamma: 3, delta: 4}\nsum = 0\ni = 0\nwhile i < 100\n  sum += record.alpha + record.beta + record.gamma + record.delta\n  i++\nsum",
        expected: "1000",
    },
    Workload {
        name: "negative-indexing",
        source: "text = 'a☕中'\nitems = [10, 20, 30]\nitems[-1] + len(text[-2])",
        expected: "31",
    },
    Workload {
        name: "stepped-string-iteration",
        source: "sum = 0\nfor character, index in 'a☕中x' by 2 then sum += index\nsum",
        expected: "2",
    },
    Workload {
        name: "signed-by-iteration",
        source: "sum = 0\nfor n, index in [1...100] by -3 then sum += n + index\nsum",
        expected: "3333",
    },
    Workload {
        name: "postfix-loops",
        source: "sum = 0\ni = 0\ni = i + 1 while i < 100\nsum + i",
        expected: "100",
    },
    Workload {
        name: "array-slices",
        source: "items = [0...100]\nsum = 0\ni = 0\nwhile i < 100\n  slice = items[10...90]\n  sum = sum + slice[0] + slice[79]\n  i = i + 1\nsum",
        expected: "9900",
    },
    Workload {
        name: "existence-tests",
        source: "value = nil\nsum = 0\ni = 0\nwhile i < 100\n  sum = sum + (if value? then 0 else 1)\n  i = i + 1\nsum",
        expected: "100",
    },
    Workload {
        name: "existential-assignment",
        source: "value = 1\nsum = 0\ni = 0\nwhile i < 100\n  value ?= 2\n  sum = sum + value\n  i = i + 1\nsum",
        expected: "100",
    },
    Workload {
        name: "name-updates",
        source: "i = 0\nsum = 0\nwhile i < 100\n  sum += i\n  i++\nsum",
        expected: "4950",
    },
    Workload {
        name: "exact-integer-updates",
        source: "value = 9007199254740993n\ni = 0\nwhile i < 100\n  value += 7n\n  i++\nvalue",
        expected: "9007199254741693n",
    },
    Workload {
        name: "exact-decimal-money",
        source: "total = 0m\ni = 0\nwhile i < 100\n  total += 0.01m\n  i++\ndecimal_div(total * 17.5m, 100m, 2, 'half_even')",
        expected: "0.18m",
    },
    Workload {
        name: "floor-modulo",
        source: "sum = 0\ni = -100\nwhile i < 100\n  sum += i // 3\n  sum += i %% 7\n  i += 1\nsum",
        expected: "500",
    },
    Workload {
        name: "bitwise",
        source: "sum = 0\ni = -100\nwhile i < 100\n  sum += (i & 31) ^ (i << 1)\n  i += 1\nsum",
        expected: "-196",
    },
    Workload {
        name: "multiline-strings",
        source: "message = \"alpha\n  beta\"\nlen(message)",
        expected: "10",
    },
    Workload {
        name: "string-iteration",
        source: "sum = 0\nfor character, index in 'a☕中' then sum += index\nsum",
        expected: "3",
    },
    Workload {
        name: "string-escapes",
        source: "message = \"A\\x42\\u{43}\"\nlen(message) + (if message == 'ABC' then 1 else 0)",
        expected: "4",
    },
    Workload {
        name: "string-indexing",
        source: "text = 'a☕中'\nsum = 0\ni = 0\nwhile i < 100\n  sum += len(text[1..2]) + (if text[1] == '☕' then 1 else 0)\n  i += 1\nsum",
        expected: "300",
    },
    Workload {
        name: "multiline-collections",
        source: "values = [\n  1\n  2\n  3\n]\nrecord = {\n  first: 1\n  second: 2\n}\nvalues[2] + record.first + record.second",
        expected: "6",
    },
    Workload {
        name: "indented-maps",
        source: "record =\n  first: 1\n  nested:\n    second: 2\nrecord.nested.second + record.first",
        expected: "3",
    },
    Workload {
        name: "implicit-calls",
        source: "add = (left, right) -> left + right\nanswer = add 20, 22\nanswer",
        expected: "42",
    },
    Workload {
        name: "execution-stats",
        source: "sum = 0\ni = 0\nwhile i < 100\n  sum += i\n  i++\nsum",
        expected: "4950",
    },
    Workload {
        name: "constant-folding",
        source: "value = (1 + 2 * 3) == 7\nvalue",
        expected: "true",
    },
    Workload {
        name: "bare-lambda",
        source: "base = 1\nadd = n -> n + base\nsum = 0\nfor n in [1...50] then sum = sum + add(n)\nsum",
        expected: "1274",
    },
    Workload {
        name: "stepped-iteration",
        source: "sum = 0\nfor n in [1...100] by 3 then sum = sum + n\nsum",
        expected: "1617",
    },
    Workload {
        name: "for-collection",
        source: "values = for n in [1...100] when n % 3 == 0 then n * 2\nlen(values)",
        expected: "33",
    },
    Workload {
        name: "postfix-comprehension",
        source: "values = n * 2 for n in [1...100]\nsum = 0\nfor n in values then sum = sum + n\nsum",
        expected: "9900",
    },
    Workload {
        name: "for-pattern-bindings",
        source: "pairs = for n in [1...100] then [n, n + 1]\nsum = 0\nfor [left, right] in pairs then sum = sum + left + right\nsum",
        expected: "9999",
    },
    Workload {
        name: "maps-and-control",
        source: "record = {a: 1, b: 2, c: 3}\nsum = 0\nfor own key, value of record when value > 1 then sum = sum + value\ntry sum ? 0 catch error then 0",
        expected: "5",
    },
    Workload {
        name: "soak-access",
        source: "record = {answer: 1}\nnone = nil\nsum = 0\ni = 0\nwhile i < 100\n  sum = sum + record?.answer + (none?[i] ? 0)\n  i = i + 1\nsum",
        expected: "100",
    },
    Workload {
        name: "nested-destructuring",
        source: "sum = 0\ni = 0\nwhile i < 100\n  [first, {point: [x, y]}] = [1, {point: [2, 3]}]\n  sum = sum + first + x + y\n  i = i + 1\nsum",
        expected: "600",
    },
    Workload {
        name: "destructuring-rest",
        source: "sum = 0\ni = 0\nwhile i < 100\n  [head, tail...] = [1, 2, 3, 4]\n  sum += head + len(tail)\n  i += 1\nsum",
        expected: "400",
    },
    Workload {
        name: "chained-comparisons",
        source: "low = 0\nmiddle = 1\nhigh = 2\nsum = 0\ni = 0\nwhile i < 100\n  sum = sum + (if low < middle < high then 1 else 0)\n  i = i + 1\nsum",
        expected: "100",
    },
    Workload {
        name: "destructuring-parameters",
        source: "scale = ([left, right], {factor}) -> (left + right) * factor\nsum = 0\ni = 0\nwhile i < 100\n  sum = sum + scale([1, 2], {factor: 3})\n  i = i + 1\nsum",
        expected: "900",
    },
    Workload {
        name: "return-cleanup",
        source: "find = (items) ->\n  try\n    for n in items then if n == 73 then return n\n    nil\n  catch error\n    0\n  finally\n    0\nsum = 0\ni = 0\nwhile i < 100\n  sum = sum + find([1...100])\n  i = i + 1\nsum",
        expected: "7300",
    },
];

fn usage() {
    eprintln!(
        "Usage: qbench [--iterations N] [--repeat N] [--only NAME] [--json]\n       qbench --compare-qjs PATH [--compare-iterations N] [--repeat N] [--json]\n       qbench --list\n       qbench --version"
    );
}

fn cli_binary(name: &str) -> Result<std::path::PathBuf, String> {
    let current = env::current_exe().map_err(|error| error.to_string())?;
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    let path = current.with_file_name(file);
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "{} is not beside qbench; build or install both CLI binaries",
            path.display()
        ))
    }
}

fn run_checked(command: &mut Command, expected: &str) -> Result<(), String> {
    let output = command.output().map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    if String::from_utf8_lossy(&output.stdout).trim() != expected {
        return Err(format!(
            "expected {expected}, got {}",
            String::from_utf8_lossy(&output.stdout).trim()
        ));
    }
    Ok(())
}

fn parse_phase_output(output: &[u8]) -> Result<(u128, u128), String> {
    let text = String::from_utf8_lossy(output);
    let mut fields = text.split_whitespace();
    let compile_ns = fields
        .next()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| "QuickJS phase output is missing compile nanoseconds".to_owned())?;
    let hot_ns = fields
        .next()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| "QuickJS phase output is missing hot-execution nanoseconds".to_owned())?;
    if fields.next().is_some() {
        return Err("QuickJS phase output contains unexpected fields".to_owned());
    }
    Ok((compile_ns, hot_ns))
}

fn run_quickjs_phases(
    path: &str,
    workload: &ComparisonWorkload,
    iterations: usize,
) -> Result<(u128, u128), String> {
    let script = format!(
        "const source=\"{}\";const expected=\"{}\";let workload;let start=os.now();for(let i=0;i<{};i++)workload=std.evalScript(source);const compileNs=Math.round((os.now()-start)*1000000);start=os.now();for(let i=0;i<{};i++){{const value=workload();if(String(value)!==expected)throw new Error(`expected ${{expected}}, got ${{value}}`);}}const hotNs=Math.round((os.now()-start)*1000000);print(`${{compileNs}} ${{hotNs}}`);",
        json_escape(workload.quickjs),
        json_escape(workload.expected),
        iterations,
        iterations
    );
    let output = Command::new(path)
        .args(["--std", "-e", &script])
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    parse_phase_output(&output.stdout)
}

fn measure_startup(
    executable: &std::ffi::OsStr,
    quit_argument: &str,
    iterations: usize,
    repeat: usize,
) -> Result<(u128, u128), String> {
    let mut samples = Vec::with_capacity(repeat);
    for _ in 0..repeat {
        let start = Instant::now();
        for _ in 0..iterations {
            run_checked(Command::new(executable).arg(quit_argument), "")?;
        }
        samples.push(start.elapsed().as_nanos());
    }
    Ok(median_and_mad(&mut samples))
}

fn compare_qjs(path: &str, iterations: usize, repeat: usize, json: bool) -> Result<(), String> {
    let qcoffee = cli_binary("qcoffee")?;
    let (quickcoffee_startup_ns, quickcoffee_startup_mad_ns) =
        measure_startup(qcoffee.as_os_str(), "--quit", iterations, repeat)?;
    let (quickjs_startup_ns, quickjs_startup_mad_ns) =
        measure_startup(std::ffi::OsStr::new(path), "--quit", iterations, repeat)?;
    let engine = Engine::new();
    for workload in COMPARISON_WORKLOADS {
        let mut quickcoffee_samples = Vec::with_capacity(repeat);
        let mut quickjs_samples = Vec::with_capacity(repeat);
        let mut quickcoffee_compile_samples = Vec::with_capacity(repeat);
        let mut quickcoffee_hot_samples = Vec::with_capacity(repeat);
        let mut quickjs_compile_samples = Vec::with_capacity(repeat);
        let mut quickjs_hot_samples = Vec::with_capacity(repeat);
        let hot_program = engine
            .compile_program(workload.quickcoffee)
            .map_err(|error| error.to_string())?;
        let quickjs_cli_source = format!("console.log(({})());", workload.quickjs);
        for _ in 0..repeat {
            let start = Instant::now();
            for _ in 0..iterations {
                run_checked(
                    Command::new(&qcoffee).args(["--fuel", "20000000", "-e", workload.quickcoffee]),
                    workload.expected,
                )?;
            }
            quickcoffee_samples.push(start.elapsed().as_nanos());

            let start = Instant::now();
            for _ in 0..iterations {
                run_checked(
                    Command::new(path).args(["-e", &quickjs_cli_source]),
                    workload.expected,
                )?;
            }
            quickjs_samples.push(start.elapsed().as_nanos());

            let start = Instant::now();
            for _ in 0..iterations {
                engine
                    .compile(workload.quickcoffee)
                    .map_err(|error| error.to_string())?;
            }
            quickcoffee_compile_samples.push(start.elapsed().as_nanos());

            let mut context = Context::new().with_fuel(20_000_000);
            let start = Instant::now();
            for _ in 0..iterations {
                let value = context
                    .run_program(&hot_program)
                    .map_err(|error| error.to_string())?;
                if value.to_string() != workload.expected {
                    return Err(format!("expected {}, got {value}", workload.expected));
                }
            }
            quickcoffee_hot_samples.push(start.elapsed().as_nanos());

            let (compile_ns, hot_ns) = run_quickjs_phases(path, workload, iterations)?;
            quickjs_compile_samples.push(compile_ns);
            quickjs_hot_samples.push(hot_ns);
        }
        let (quickcoffee_ns, quickcoffee_mad_ns) = median_and_mad(&mut quickcoffee_samples);
        let (quickjs_ns, quickjs_mad_ns) = median_and_mad(&mut quickjs_samples);
        let (quickcoffee_compile_ns, quickcoffee_compile_mad_ns) =
            median_and_mad(&mut quickcoffee_compile_samples);
        let (quickcoffee_hot_ns, quickcoffee_hot_mad_ns) =
            median_and_mad(&mut quickcoffee_hot_samples);
        let (quickjs_compile_ns, quickjs_compile_mad_ns) =
            median_and_mad(&mut quickjs_compile_samples);
        let (quickjs_hot_ns, quickjs_hot_mad_ns) = median_and_mad(&mut quickjs_hot_samples);
        if json {
            println!(
                "{{\"schema\":\"{COMPARISON_SCHEMA}\",\"name\":\"{}\",\"iterations\":{},\"repeat\":{},\"expected\":\"{}\",\"quickcoffee_startup_ns\":{},\"quickcoffee_startup_mad_ns\":{},\"quickjs_startup_ns\":{},\"quickjs_startup_mad_ns\":{},\"quickcoffee_compile_ns\":{},\"quickcoffee_compile_mad_ns\":{},\"quickjs_compile_ns\":{},\"quickjs_compile_mad_ns\":{},\"quickcoffee_hot_ns\":{},\"quickcoffee_hot_mad_ns\":{},\"quickjs_hot_ns\":{},\"quickjs_hot_mad_ns\":{},\"quickcoffee_cli_ns\":{},\"quickcoffee_cli_mad_ns\":{},\"quickjs_cli_ns\":{},\"quickjs_cli_mad_ns\":{}}}",
                workload.name,
                iterations,
                repeat,
                workload.expected,
                quickcoffee_startup_ns,
                quickcoffee_startup_mad_ns,
                quickjs_startup_ns,
                quickjs_startup_mad_ns,
                quickcoffee_compile_ns,
                quickcoffee_compile_mad_ns,
                quickjs_compile_ns,
                quickjs_compile_mad_ns,
                quickcoffee_hot_ns,
                quickcoffee_hot_mad_ns,
                quickjs_hot_ns,
                quickjs_hot_mad_ns,
                quickcoffee_ns,
                quickcoffee_mad_ns,
                quickjs_ns,
                quickjs_mad_ns
            );
        } else {
            println!(
                "schema={COMPARISON_SCHEMA} {} iterations={} repeat={} quickcoffee_startup_ns={} quickcoffee_startup_mad_ns={} quickjs_startup_ns={} quickjs_startup_mad_ns={} quickcoffee_compile_ns={} quickcoffee_compile_mad_ns={} quickjs_compile_ns={} quickjs_compile_mad_ns={} quickcoffee_hot_ns={} quickcoffee_hot_mad_ns={} quickjs_hot_ns={} quickjs_hot_mad_ns={} quickcoffee_cli_ns={} quickcoffee_cli_mad_ns={} quickjs_cli_ns={} quickjs_cli_mad_ns={} expected={}",
                workload.name,
                iterations,
                repeat,
                quickcoffee_startup_ns,
                quickcoffee_startup_mad_ns,
                quickjs_startup_ns,
                quickjs_startup_mad_ns,
                quickcoffee_compile_ns,
                quickcoffee_compile_mad_ns,
                quickjs_compile_ns,
                quickjs_compile_mad_ns,
                quickcoffee_hot_ns,
                quickcoffee_hot_mad_ns,
                quickjs_hot_ns,
                quickjs_hot_mad_ns,
                quickcoffee_ns,
                quickcoffee_mad_ns,
                quickjs_ns,
                quickjs_mad_ns,
                workload.expected
            );
        }
    }
    Ok(())
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn median(samples: &mut [u128]) -> u128 {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn median_and_mad(samples: &mut [u128]) -> (u128, u128) {
    let center = median(samples);
    for sample in &mut *samples {
        *sample = sample.abs_diff(center);
    }
    (center, median(samples))
}

fn main() -> ExitCode {
    let mut iterations = 100;
    let mut repeat = 1;
    let mut json = false;
    let mut only = None;
    let mut list = false;
    let mut compare_qjs_path = None;
    let mut compare_iterations = 1;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" => {
                println!("qbench {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--help" | "-h" => {
                usage();
                return ExitCode::SUCCESS;
            }
            "--list" => list = true,
            "--compare-qjs" => match args.next() {
                Some(path) if !path.is_empty() => compare_qjs_path = Some(path),
                _ => {
                    eprintln!("--compare-qjs requires a qjs executable path");
                    return ExitCode::from(2);
                }
            },
            "--compare-iterations" => match args.next().and_then(|value| value.parse().ok()) {
                Some(value) if value > 0 => compare_iterations = value,
                _ => {
                    eprintln!("--compare-iterations requires a positive integer");
                    return ExitCode::from(2);
                }
            },
            "--json" => json = true,
            "--only" => match args.next() {
                Some(value) if !value.is_empty() => only = Some(value),
                _ => {
                    eprintln!("--only requires a workload name");
                    return ExitCode::from(2);
                }
            },
            "--iterations" => match args.next().and_then(|value| value.parse().ok()) {
                Some(value) if value > 0 => iterations = value,
                _ => {
                    eprintln!("--iterations requires a positive integer");
                    return ExitCode::from(2);
                }
            },
            "--repeat" => match args.next().and_then(|value| value.parse().ok()) {
                Some(value) if value > 0 => repeat = value,
                _ => {
                    eprintln!("--repeat requires a positive integer");
                    return ExitCode::from(2);
                }
            },
            _ => {
                usage();
                return ExitCode::from(2);
            }
        }
    }
    if list {
        if only.is_some() || compare_qjs_path.is_some() {
            eprintln!("--list cannot be combined with execution modes");
            return ExitCode::from(2);
        }
        for workload in WORKLOADS {
            println!("{}", workload.name);
        }
        return ExitCode::SUCCESS;
    }
    if let Some(path) = compare_qjs_path {
        if only.is_some() {
            eprintln!("--compare-qjs cannot be combined with --only");
            return ExitCode::from(2);
        }
        return match compare_qjs(&path, compare_iterations, repeat, json) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("QuickJS comparison failed: {error}");
                ExitCode::from(1)
            }
        };
    }
    let workloads: Vec<&Workload> = match only.as_deref() {
        Some(name) => match WORKLOADS.iter().find(|workload| workload.name == name) {
            Some(workload) => vec![workload],
            None => {
                eprintln!("unknown workload '{name}'; use --list to see available workloads");
                return ExitCode::from(2);
            }
        },
        None => WORKLOADS.iter().collect(),
    };
    let engine = Engine::new();
    for workload in workloads {
        let mut compile_samples = Vec::with_capacity(repeat);
        let mut prepare_samples = Vec::with_capacity(repeat);
        let mut verify_samples = Vec::with_capacity(repeat);
        let mut execute_samples = Vec::with_capacity(repeat);
        let mut prepared_program = None;
        for _ in 0..repeat {
            let start = Instant::now();
            for _ in 0..iterations {
                engine
                    .compile(workload.source)
                    .expect("qbench workload must compile");
            }
            compile_samples.push(start.elapsed().as_nanos());

            let start = Instant::now();
            for _ in 0..iterations {
                prepared_program = Some(
                    engine
                        .compile_program(workload.source)
                        .expect("qbench workload must prepare"),
                );
            }
            prepare_samples.push(start.elapsed().as_nanos());
            let program = prepared_program
                .as_ref()
                .expect("positive iterations prepare a program");

            let start = Instant::now();
            for _ in 0..iterations {
                program.verify().expect("qbench workload must verify");
            }
            verify_samples.push(start.elapsed().as_nanos());

            let start = Instant::now();
            for _ in 0..iterations {
                let mut context = Context::new().with_fuel(100_000);
                let value = context
                    .run_program(program)
                    .expect("qbench workload must execute");
                assert_eq!(value.to_string(), workload.expected, "{}", workload.name);
            }
            execute_samples.push(start.elapsed().as_nanos());
        }
        let (compile_ns, compile_mad_ns) = median_and_mad(&mut compile_samples);
        let (prepare_ns, prepare_mad_ns) = median_and_mad(&mut prepare_samples);
        let (verify_ns, verify_mad_ns) = median_and_mad(&mut verify_samples);
        let (execute_ns, execute_mad_ns) = median_and_mad(&mut execute_samples);
        let profile_program = prepared_program.expect("positive repeat prepares a program");
        let mut profile_context = Context::new().with_fuel(100_000);
        let profile_value = profile_context
            .run_program(&profile_program)
            .expect("qbench workload must execute");
        assert_eq!(
            profile_value.to_string(),
            workload.expected,
            "{}",
            workload.name
        );
        let profile = profile_context.last_execution();

        if json {
            println!(
                "{{\"schema\":\"{}\",\"version\":\"{}\",\"name\":\"{}\",\"iterations\":{},\"repeat\":{},\"expected\":\"{}\",\"compile_ns\":{},\"compile_mad_ns\":{},\"prepare_ns\":{},\"prepare_mad_ns\":{},\"verify_ns\":{},\"verify_mad_ns\":{},\"execute_ns\":{},\"execute_mad_ns\":{},\"profile_instructions\":{},\"profile_call_depth_peak\":{},\"profile_name_loads\":{},\"profile_name_stores\":{},\"profile_calls\":{},\"profile_container_ops\":{},\"profile_iterator_ops\":{},\"profile_exception_ops\":{},\"profile_value_allocations\":{},\"profile_environment_allocations\":{}}}",
                OUTPUT_SCHEMA,
                env!("CARGO_PKG_VERSION"),
                json_escape(workload.name),
                iterations,
                repeat,
                json_escape(workload.expected),
                compile_ns,
                compile_mad_ns,
                prepare_ns,
                prepare_mad_ns,
                verify_ns,
                verify_mad_ns,
                execute_ns,
                execute_mad_ns,
                profile.instructions,
                profile.call_depth_peak,
                profile.name_loads,
                profile.name_stores,
                profile.calls,
                profile.container_ops,
                profile.iterator_ops,
                profile.exception_ops,
                profile.value_allocations,
                profile.environment_allocations
            );
        } else {
            println!(
                "schema={} version={} {} iterations={} repeat={} compile_ns={} compile_mad_ns={} prepare_ns={} prepare_mad_ns={} verify_ns={} verify_mad_ns={} execute_ns={} execute_mad_ns={} profile_instructions={} profile_call_depth_peak={} profile_name_loads={} profile_name_stores={} profile_calls={} profile_container_ops={} profile_iterator_ops={} profile_exception_ops={} profile_value_allocations={} profile_environment_allocations={} expected={}",
                OUTPUT_SCHEMA,
                env!("CARGO_PKG_VERSION"),
                workload.name,
                iterations,
                repeat,
                compile_ns,
                compile_mad_ns,
                prepare_ns,
                prepare_mad_ns,
                verify_ns,
                verify_mad_ns,
                execute_ns,
                execute_mad_ns,
                profile.instructions,
                profile.call_depth_peak,
                profile.name_loads,
                profile.name_stores,
                profile.calls,
                profile.container_ops,
                profile.iterator_ops,
                profile.exception_ops,
                profile.value_allocations,
                profile.environment_allocations,
                workload.expected
            );
        }
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::parse_phase_output;

    #[test]
    fn quickjs_phase_output_requires_exact_unsigned_fields() {
        assert_eq!(parse_phase_output(b"10 20\n").unwrap(), (10, 20));
        assert!(parse_phase_output(b"10\n").is_err());
        assert!(parse_phase_output(b"ten 20\n").is_err());
        assert!(parse_phase_output(b"10 20 30\n").is_err());
    }
}

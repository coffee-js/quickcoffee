use quickcoffee::{Context, Engine, Program};
use std::{env, process::ExitCode, time::Instant};

struct Workload {
    name: &'static str,
    source: &'static str,
    expected: &'static str,
}

const WORKLOADS: &[Workload] = &[
    Workload {
        name: "loop-core",
        source: "sum = 0\ni = 0\nwhile i < 100 then i = i + 1\nsum + i",
        expected: "100",
    },
    Workload {
        name: "closures-and-ranges",
        source: "base = 1\nadd = (n) -> n + base\nsum = 0\nfor n in [1...50] then sum = sum + add(n)\nsum",
        expected: "1274",
    },
    Workload {
        name: "map-spread",
        source: "base = {a: 1, b: 2}\nout = {...base, b: 3, c: 4}\nout.a + out.b + out.c",
        expected: "8",
    },
    Workload {
        name: "negative-indexing",
        source: "text = 'a☕中'\nitems = [10, 20, 30]\nitems[-1] + len(text[-2])",
        expected: "31",
    },
];

fn usage() {
    eprintln!("Usage: qbench [--iterations N] [--json]");
}

fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            character if character.is_control() => {
                out.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => out.push(character),
        }
    }
    out
}

fn main() -> ExitCode {
    let mut iterations = 100;
    let mut json = false;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                usage();
                return ExitCode::SUCCESS;
            }
            "--json" => json = true,
            "--iterations" => match args.next().and_then(|value| value.parse().ok()) {
                Some(value) if value > 0 => iterations = value,
                _ => {
                    eprintln!("--iterations requires a positive integer");
                    return ExitCode::from(2);
                }
            },
            _ => {
                usage();
                return ExitCode::from(2);
            }
        }
    }
    let engine = Engine::new();
    for workload in WORKLOADS {
        let start = Instant::now();
        for _ in 0..iterations {
            engine
                .compile_unverified(workload.source)
                .expect("qbench workload must compile");
        }
        let compile_ns = start.elapsed().as_nanos();
        let program = engine
            .compile_unverified(workload.source)
            .expect("qbench workload must compile");
        let program: Program = program.into();

        let start = Instant::now();
        for _ in 0..iterations {
            program.verify().expect("qbench workload must verify");
        }
        let verify_ns = start.elapsed().as_nanos();

        let start = Instant::now();
        for _ in 0..iterations {
            let mut context = Context::new();
            let value = context
                .run_program(&program)
                .expect("qbench workload must execute");
            assert_eq!(value.to_string(), workload.expected, "{}", workload.name);
        }
        let execute_ns = start.elapsed().as_nanos();

        if json {
            println!(
                "{{\"name\":\"{}\",\"iterations\":{},\"expected\":\"{}\",\"compile_ns\":{},\"verify_ns\":{},\"execute_ns\":{}}}",
                json_escape(workload.name),
                iterations,
                json_escape(workload.expected),
                compile_ns,
                verify_ns,
                execute_ns
            );
        } else {
            println!(
                "{} iterations={} compile_ns={} verify_ns={} execute_ns={} expected={}",
                workload.name, iterations, compile_ns, verify_ns, execute_ns, workload.expected
            );
        }
    }
    ExitCode::SUCCESS
}

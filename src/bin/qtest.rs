//! Test runner for executable QuickCoffee scripts and literate manuals.

use quickcoffee::{CancellationToken, Context, Value};
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::Duration,
};

fn collect(
    path: &Path,
    files: &mut Vec<PathBuf>,
    visited_directories: &mut HashSet<PathBuf>,
    visited_files: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if metadata.is_file() {
        if !is_source_file(path) {
            return Err(format!(
                "{}: expected a .coffee or .litcoffee source file",
                path.display()
            ));
        }
        let canonical =
            fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))?;
        if visited_files.insert(canonical) {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }
    if metadata.is_dir() {
        let canonical =
            fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))?;
        if !visited_directories.insert(canonical) {
            return Ok(());
        }
    }
    let mut entries: Vec<_> = fs::read_dir(path)
        .map_err(|error| format!("{}: {error}", path.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let child = entry.path();
        if child.is_dir() || is_source_file(&child) {
            collect(&child, files, visited_directories, visited_files)?;
        }
    }
    Ok(())
}
fn is_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "coffee" | "litcoffee"))
}
fn usage() {
    eprintln!(
        "Usage: qtest [--fuel N] [--timeout-ms N] [--stats] [--json|--tap] [--filter TEXT] FILE_OR_DIRECTORY...\n       qtest --list [--filter TEXT] FILE_OR_DIRECTORY...\n       qtest --version"
    );
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
fn tap_comments(detail: &str) {
    for line in detail.lines() {
        println!("# {line}");
    }
    if detail.is_empty() {
        println!("#");
    }
}

struct TestRun {
    outcome: Result<(), String>,
    instructions: Option<u64>,
    fuel_remaining: Option<u64>,
}

fn run_file(path: PathBuf, fuel: u64, cancellation: Option<CancellationToken>) -> TestRun {
    let source_name = path.display().to_string();
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => {
            return TestRun {
                outcome: Err(error.to_string()),
                instructions: None,
                fuel_remaining: None,
            };
        }
    };
    let mut context = Context::new().with_fuel(fuel);
    if let Some(cancellation) = cancellation {
        context.set_cancellation_token(cancellation);
    }
    let outcome = match context.eval_named(&source_name, &source) {
        Ok(Value::Bool(true)) => Ok(()),
        Ok(value) => Err(format!("final value was {value}, expected true")),
        Err(error) => Err(error.to_string()),
    };
    let execution = context.last_execution();
    TestRun {
        outcome,
        instructions: Some(execution.instructions),
        fuel_remaining: Some(execution.fuel_remaining),
    }
}

fn run_file_with_timeout(path: PathBuf, fuel: u64, timeout_ms: Option<u64>) -> TestRun {
    let Some(timeout_ms) = timeout_ms else {
        return run_file(path, fuel, None);
    };
    let cancellation = CancellationToken::new();
    let worker_cancellation = cancellation.clone();
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = sender.send(run_file(path, fuel, Some(worker_cancellation)));
    });
    match receiver.recv_timeout(Duration::from_millis(timeout_ms)) {
        Ok(run) => run,
        Err(RecvTimeoutError::Timeout) => {
            cancellation.cancel();
            match receiver.recv() {
                Ok(mut run) => {
                    run.outcome = Err(format!("execution timed out after {timeout_ms} ms"));
                    run
                }
                Err(_) => TestRun {
                    outcome: Err(format!("execution timed out after {timeout_ms} ms")),
                    instructions: None,
                    fuel_remaining: None,
                },
            }
        }
        Err(RecvTimeoutError::Disconnected) => TestRun {
            outcome: Err("qtest worker terminated without a result".to_string()),
            instructions: None,
            fuel_remaining: None,
        },
    }
}

fn main() -> ExitCode {
    let mut fuel = 1_000_000u64;
    let mut timeout_ms = None;
    let mut stats = false;
    let mut json = false;
    let mut tap = false;
    let mut filter = None;
    let mut list = false;
    let mut inputs: Vec<String> = vec![];
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" => {
                println!("qtest {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--help" | "-h" => {
                usage();
                return ExitCode::SUCCESS;
            }
            "--fuel" => match args.next().and_then(|value| value.parse().ok()) {
                Some(value) => fuel = value,
                None => {
                    eprintln!("--fuel requires a non-negative integer");
                    return ExitCode::from(2);
                }
            },
            "--timeout-ms" => match args.next().and_then(|value| value.parse::<u64>().ok()) {
                Some(value) if value > 0 => timeout_ms = Some(value),
                _ => {
                    eprintln!("--timeout-ms requires a positive integer");
                    return ExitCode::from(2);
                }
            },
            "--stats" => stats = true,
            "--json" => json = true,
            "--tap" => tap = true,
            "--list" => list = true,
            "--filter" => match args.next() {
                Some(value) if !value.is_empty() => filter = Some(value),
                _ => {
                    eprintln!("--filter requires non-empty text");
                    return ExitCode::from(2);
                }
            },
            value if !value.starts_with('-') => inputs.push(value.to_string()),
            _ => {
                usage();
                return ExitCode::from(2);
            }
        }
    }
    if inputs.is_empty() {
        usage();
        return ExitCode::from(2);
    }
    if json && tap {
        eprintln!("--json and --tap cannot be used together");
        return ExitCode::from(2);
    }
    let mut files = vec![];
    let mut visited_directories = HashSet::new();
    let mut visited_files = HashSet::new();
    for input in inputs {
        if let Err(error) = collect(
            Path::new(&input),
            &mut files,
            &mut visited_directories,
            &mut visited_files,
        ) {
            if tap {
                println!("TAP version 13");
                println!("Bail out! {input}: {error}");
            } else {
                eprintln!("not ok {input}: {error}");
            }
            return ExitCode::from(1);
        }
    }
    files.sort();
    files.dedup();
    let filtered = filter.is_some();
    if let Some(filter) = filter.as_deref() {
        files.retain(|path| {
            fs::canonicalize(path)
                .map(|canonical| canonical.to_string_lossy().contains(filter))
                .unwrap_or(false)
        });
    }
    if files.is_empty() {
        let message = if filtered {
            "no matching .coffee or .litcoffee test files found"
        } else {
            "no .coffee or .litcoffee test files found"
        };
        if tap {
            println!("TAP version 13");
            println!("Bail out! {message}");
        } else {
            eprintln!("{message}");
        }
        return ExitCode::from(2);
    }
    if list {
        if json || tap || stats || timeout_ms.is_some() {
            eprintln!("--list cannot be combined with --json, --tap, --stats, or --timeout-ms");
            return ExitCode::from(2);
        }
        for path in files {
            println!("{}", path.display());
        }
        return ExitCode::SUCCESS;
    }
    if tap {
        println!("TAP version 13");
    }
    let mut failed = 0;
    let total = files.len();
    for (index, path) in files.into_iter().enumerate() {
        let label = path.display().to_string();
        let run = run_file_with_timeout(path, fuel, timeout_ms);
        if stats {
            if let (Some(instructions), Some(fuel_remaining)) =
                (run.instructions, run.fuel_remaining)
            {
                eprintln!(
                    "qtest stats: {} instructions={} fuel_remaining={}",
                    label, instructions, fuel_remaining
                );
            }
        }
        match run.outcome {
            Ok(()) => {
                if json {
                    println!("{{\"ok\":true,\"file\":\"{}\"}}", json_escape(&label));
                } else if tap {
                    println!("ok {} - {label}", index + 1);
                } else {
                    println!("ok {label}");
                }
            }
            Err(detail) => {
                failed += 1;
                if json {
                    println!(
                        "{{\"ok\":false,\"file\":\"{}\",\"error\":\"{}\"}}",
                        json_escape(&label.to_string()),
                        json_escape(&detail)
                    );
                } else if tap {
                    println!("not ok {} - {label}", index + 1);
                    tap_comments(&detail);
                } else {
                    eprintln!("not ok {label}: {detail}");
                }
            }
        }
    }
    if tap {
        println!("1..{total}");
    }
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        eprintln!("{failed} test file(s) failed");
        ExitCode::from(1)
    }
}

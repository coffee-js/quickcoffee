use quickcoffee::{Context, Value};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

fn collect(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.is_file() {
        files.push(path.to_path_buf());
        return Ok(());
    }
    let mut entries: Vec<_> = fs::read_dir(path)
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let child = entry.path();
        if child.is_dir() {
            collect(&child, files)?;
        } else if child.extension().is_some_and(|extension| extension == "qc") {
            files.push(child);
        }
    }
    Ok(())
}
fn usage() {
    eprintln!(
        "Usage: qtest [--fuel N] [--stats] [--json|--tap] FILE_OR_DIRECTORY...\n       qtest --version"
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
fn main() -> ExitCode {
    let mut fuel = 1_000_000u64;
    let mut stats = false;
    let mut json = false;
    let mut tap = false;
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
            "--stats" => stats = true,
            "--json" => json = true,
            "--tap" => tap = true,
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
    for input in inputs {
        if let Err(error) = collect(Path::new(&input), &mut files) {
            if tap {
                println!("TAP version 13");
                println!("Bail out! {input}: {error}");
            } else {
                eprintln!("not ok {input}: {error}");
            }
            return ExitCode::from(1);
        }
    }
    if files.is_empty() {
        if tap {
            println!("TAP version 13");
            println!("Bail out! no .qc test files found");
        } else {
            eprintln!("no .qc test files found");
        }
        return ExitCode::from(2);
    }
    if tap {
        println!("TAP version 13");
    }
    let mut failed = 0;
    let total = files.len();
    for (index, path) in files.into_iter().enumerate() {
        let label = path.display();
        let outcome = fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|src| {
                let mut context = Context::new().with_fuel(fuel);
                let result = context.eval(&src).map_err(|e| e.to_string());
                if stats {
                    let execution = context.last_execution();
                    eprintln!(
                        "qtest stats: {} instructions={} fuel_remaining={}",
                        label, execution.instructions, execution.fuel_remaining
                    );
                }
                result
            });
        match outcome {
            Ok(Value::Bool(true)) => {
                if json {
                    println!(
                        "{{\"ok\":true,\"file\":\"{}\"}}",
                        json_escape(&label.to_string())
                    );
                } else if tap {
                    println!("ok {} - {label}", index + 1);
                } else {
                    println!("ok {label}");
                }
            }
            Ok(value) => {
                failed += 1;
                let detail = format!("final value was {value}, expected true");
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
            Err(e) => {
                failed += 1;
                let detail = e.to_string();
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

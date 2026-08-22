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
    eprintln!("Usage: qtest [--fuel N] FILE_OR_DIRECTORY...");
}
fn main() -> ExitCode {
    let mut fuel = 1_000_000u64;
    let mut inputs: Vec<String> = vec![];
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
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
    let mut files = vec![];
    for input in inputs {
        if let Err(error) = collect(Path::new(&input), &mut files) {
            eprintln!("not ok {input}: {error}");
            return ExitCode::from(1);
        }
    }
    if files.is_empty() {
        eprintln!("no .qc test files found");
        return ExitCode::from(2);
    }
    let mut failed = 0;
    for path in files {
        let label = path.display();
        let outcome = fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|src| {
                Context::new()
                    .with_fuel(fuel)
                    .eval(&src)
                    .map_err(|e| e.to_string())
            });
        match outcome {
            Ok(Value::Bool(true)) => println!("ok {label}"),
            Ok(value) => {
                failed += 1;
                eprintln!("not ok {label}: final value was {value}, expected true")
            }
            Err(e) => {
                failed += 1;
                eprintln!("not ok {label}: {e}")
            }
        }
    }
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        eprintln!("{failed} test file(s) failed");
        ExitCode::from(1)
    }
}

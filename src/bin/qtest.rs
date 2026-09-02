//! Test runner for executable QuickCoffee scripts and literate manuals.

#[path = "../cli_diagnostic.rs"]
mod cli_diagnostic;

use quickcoffee::{CancellationToken, Context, Error, RestrictedFileModuleLoader, Runtime, Value};
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

fn module_directory_path(canonical_root: &Path, input: &str) -> Result<Option<PathBuf>, String> {
    if input.is_empty()
        || input.starts_with('/')
        || input.contains(['\\', ':'])
        || input
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Ok(None);
    }
    let requested = canonical_root.join(input);
    let Ok(canonical) = fs::canonicalize(&requested) else {
        return Ok(None);
    };
    if !canonical.starts_with(canonical_root) {
        return Err(format!(
            "module test directory escapes configured root: {input}"
        ));
    }
    if canonical.is_dir() {
        Ok(Some(canonical))
    } else {
        Ok(None)
    }
}

fn collect_module_directory(
    root: &Path,
    directory: &Path,
    entries: &mut Vec<String>,
    visited_directories: &mut HashSet<PathBuf>,
    visited_files: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    let canonical_directory =
        fs::canonicalize(directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    if !canonical_directory.starts_with(root) {
        return Err(format!(
            "module test directory escapes configured root: {}",
            directory.display()
        ));
    }
    if !visited_directories.insert(canonical_directory.clone()) {
        return Ok(());
    }
    let mut children: Vec<_> = fs::read_dir(&canonical_directory)
        .map_err(|error| format!("{}: {error}", canonical_directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("{}: {error}", canonical_directory.display()))?;
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        let path = child.path();
        let metadata =
            fs::metadata(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        if metadata.is_dir() {
            collect_module_directory(root, &path, entries, visited_directories, visited_files)?;
        } else if metadata.is_file() && is_source_file(&path) {
            let canonical =
                fs::canonicalize(&path).map_err(|error| format!("{}: {error}", path.display()))?;
            if !canonical.starts_with(root) {
                return Err(format!(
                    "module test file escapes configured root: {}",
                    path.display()
                ));
            }
            if visited_files.insert(canonical.clone()) {
                let relative = canonical.strip_prefix(root).map_err(|_| {
                    format!(
                        "module test file escapes configured root: {}",
                        path.display()
                    )
                })?;
                entries.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    Ok(())
}

fn discover_module_directory(
    canonical_root: &Path,
    input: &str,
) -> Result<Option<Vec<String>>, String> {
    let Some(directory) = module_directory_path(canonical_root, input)? else {
        return Ok(None);
    };
    let mut entries = Vec::new();
    collect_module_directory(
        canonical_root,
        &directory,
        &mut entries,
        &mut HashSet::new(),
        &mut HashSet::new(),
    )?;
    Ok(Some(entries))
}

fn is_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "coffee" | "litcoffee"))
}
fn usage() {
    eprintln!(
        "Usage: qtest [--fuel N] [--timeout-ms N] [--junit FILE] [--stats] [--json|--tap] [--filter TEXT] FILE_OR_DIRECTORY...\n       qtest [OPTIONS] --module-root ROOT ENTRY_OR_DIRECTORY...\n       qtest --list [--filter TEXT] FILE_OR_DIRECTORY...\n       qtest --list [--filter TEXT] --module-root ROOT ENTRY_OR_DIRECTORY...\n       qtest --version"
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

#[derive(Clone)]
enum TestCase {
    File(PathBuf),
    Module { root: PathBuf, entry: String },
}
impl TestCase {
    fn label(&self) -> String {
        match self {
            Self::File(path) => path.display().to_string(),
            Self::Module { entry, .. } => entry.clone(),
        }
    }
}

struct JunitCase {
    name: String,
    failure: Option<String>,
}

fn xml_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '\'' => escaped.push_str("&apos;"),
            '"' => escaped.push_str("&quot;"),
            '\u{0}'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' => escaped.push('\u{fffd}'),
            character => escaped.push(character),
        }
    }
    escaped
}

fn write_junit(path: &Path, cases: &[JunitCase]) -> Result<(), String> {
    let failures = cases.iter().filter(|case| case.failure.is_some()).count();
    let mut report = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuite name=\"qtest\" tests=\"{}\" failures=\"{failures}\" errors=\"0\" skipped=\"0\">\n",
        cases.len()
    );
    for case in cases {
        report.push_str("  <testcase name=\"");
        report.push_str(&xml_escape(&case.name));
        report.push_str("\">");
        if let Some(failure) = &case.failure {
            report.push_str("\n    <failure message=\"");
            report.push_str(&xml_escape(failure));
            report.push_str("\">");
            report.push_str(&xml_escape(failure));
            report.push_str("</failure>\n  ");
        }
        report.push_str("</testcase>\n");
    }
    report.push_str("</testsuite>\n");
    fs::write(path, report).map_err(|error| format!("{}: {error}", path.display()))
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
        Err(error) => Err(cli_diagnostic::render_error(&error, None, |requested| {
            (requested == source_name).then(|| source.clone())
        })),
    };
    let execution = context.last_execution();
    TestRun {
        outcome,
        instructions: Some(execution.instructions),
        fuel_remaining: Some(execution.fuel_remaining),
    }
}

fn module_error_detail(
    error: &Error,
    loader: &cli_diagnostic::RecordingModuleLoader<'_>,
) -> String {
    cli_diagnostic::render_error(error, None, |source_name| loader.source(source_name))
}

fn prepare_module_case(
    loader: &RestrictedFileModuleLoader,
    diagnostic_loader: &cli_diagnostic::RecordingModuleLoader<'_>,
    entry: &str,
) -> Result<(Runtime, quickcoffee::ModulePackage), Error> {
    let source = loader.load_entry(entry)?;
    diagnostic_loader.record(&source);
    let runtime = Runtime::new();
    let module = runtime.compile_module(source.name(), source.source())?;
    let package = runtime.prepare_module_package(&module, diagnostic_loader)?;
    Ok((runtime, package))
}

fn run_module(
    root: PathBuf,
    entry: String,
    fuel: u64,
    cancellation: Option<CancellationToken>,
) -> TestRun {
    let loader = match RestrictedFileModuleLoader::new(&root) {
        Ok(loader) => loader,
        Err(error) => {
            return TestRun {
                outcome: Err(cli_diagnostic::render_error(&error, None, |_| None)),
                instructions: None,
                fuel_remaining: None,
            };
        }
    };
    let diagnostic_loader = cli_diagnostic::RecordingModuleLoader::new(&loader);
    let (runtime, package) = match prepare_module_case(&loader, &diagnostic_loader, &entry) {
        Ok(prepared) => prepared,
        Err(error) => {
            return TestRun {
                outcome: Err(module_error_detail(&error, &diagnostic_loader)),
                instructions: None,
                fuel_remaining: None,
            };
        }
    };
    let mut context = runtime.new_context().with_fuel(fuel);
    if let Some(cancellation) = cancellation {
        context.set_cancellation_token(cancellation);
    }
    let outcome = match context.run_module_package(&package) {
        Ok(exports) => match exports.get("test") {
            Some(Value::Bool(true)) => Ok(()),
            Some(value) => Err(format!("export test was {value}, expected true")),
            None => Err("module did not export test, expected true".to_owned()),
        },
        Err(error) => Err(module_error_detail(&error, &diagnostic_loader)),
    };
    let execution = context.last_execution();
    TestRun {
        outcome,
        instructions: Some(execution.instructions),
        fuel_remaining: Some(execution.fuel_remaining),
    }
}

fn run_case(case: TestCase, fuel: u64, cancellation: Option<CancellationToken>) -> TestRun {
    match case {
        TestCase::File(path) => run_file(path, fuel, cancellation),
        TestCase::Module { root, entry } => run_module(root, entry, fuel, cancellation),
    }
}

fn run_case_with_timeout(case: TestCase, fuel: u64, timeout_ms: Option<u64>) -> TestRun {
    let Some(timeout_ms) = timeout_ms else {
        return run_case(case, fuel, None);
    };
    let cancellation = CancellationToken::new();
    let worker_cancellation = cancellation.clone();
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = sender.send(run_case(case, fuel, Some(worker_cancellation)));
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
    let mut junit = None;
    let mut stats = false;
    let mut json = false;
    let mut tap = false;
    let mut filter = None;
    let mut list = false;
    let mut module_root = None;
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
            "--junit" => match args.next() {
                Some(path) if !path.is_empty() => junit = Some(PathBuf::from(path)),
                _ => {
                    eprintln!("--junit requires a non-empty output path");
                    return ExitCode::from(2);
                }
            },
            "--stats" => stats = true,
            "--json" => json = true,
            "--tap" => tap = true,
            "--list" => list = true,
            "--module-root" => match args.next() {
                Some(root) if module_root.is_none() => module_root = Some(PathBuf::from(root)),
                Some(_) => {
                    eprintln!("--module-root may be specified only once");
                    return ExitCode::from(2);
                }
                None => {
                    eprintln!("--module-root requires a root directory");
                    return ExitCode::from(2);
                }
            },
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
    let mut cases = if let Some(root) = module_root {
        let root = match fs::canonicalize(&root) {
            Ok(root) => root,
            Err(error) => {
                if tap {
                    println!("TAP version 13");
                    println!("Bail out! {}: {error}", root.display());
                } else {
                    eprintln!("not ok {}: {error}", root.display());
                }
                return ExitCode::from(1);
            }
        };
        let loader = match RestrictedFileModuleLoader::new(&root) {
            Ok(loader) => loader,
            Err(error) => {
                if tap {
                    println!("TAP version 13");
                    println!("Bail out! {}: {error}", root.display());
                } else {
                    eprintln!("not ok {}: {error}", root.display());
                }
                return ExitCode::from(1);
            }
        };
        let mut cases = Vec::new();
        let mut visited_modules = HashSet::new();
        for input in inputs {
            let entries = match discover_module_directory(&root, &input) {
                Ok(Some(entries)) => entries,
                Ok(None) => match loader.load_entry(&input) {
                    Ok(source) => vec![source.name().to_owned()],
                    Err(error) => {
                        if tap {
                            println!("TAP version 13");
                            println!("Bail out! {input}: {error}");
                        } else {
                            eprintln!("not ok {input}: {error}");
                        }
                        return ExitCode::from(1);
                    }
                },
                Err(error) => {
                    if tap {
                        println!("TAP version 13");
                        println!("Bail out! {input}: {error}");
                    } else {
                        eprintln!("not ok {input}: {error}");
                    }
                    return ExitCode::from(1);
                }
            };
            for entry in entries {
                let source = match loader.load_entry(&entry) {
                    Ok(source) => source,
                    Err(error) => {
                        if tap {
                            println!("TAP version 13");
                            println!("Bail out! {entry}: {error}");
                        } else {
                            eprintln!("not ok {entry}: {error}");
                        }
                        return ExitCode::from(1);
                    }
                };
                let entry = source.name().to_owned();
                if visited_modules.insert(entry.clone()) {
                    cases.push(TestCase::Module {
                        root: root.clone(),
                        entry,
                    });
                }
            }
        }
        cases
    } else {
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
        files.into_iter().map(TestCase::File).collect()
    };
    cases.sort_by_key(TestCase::label);
    let filtered = filter.is_some();
    if let Some(filter) = filter.as_deref() {
        cases.retain(|case| match case {
            TestCase::File(path) => fs::canonicalize(path)
                .map(|canonical| canonical.to_string_lossy().contains(filter))
                .unwrap_or(false),
            TestCase::Module { entry, .. } => entry.contains(filter),
        });
    }
    if cases.is_empty() {
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
        if json || tap || stats || timeout_ms.is_some() || junit.is_some() {
            eprintln!(
                "--list cannot be combined with --json, --tap, --stats, --timeout-ms, or --junit"
            );
            return ExitCode::from(2);
        }
        for case in cases {
            println!("{}", case.label());
        }
        return ExitCode::SUCCESS;
    }
    if tap {
        println!("TAP version 13");
    }
    let mut failed = 0;
    let total = cases.len();
    let mut junit_cases = junit.as_ref().map(|_| Vec::with_capacity(total));
    for (index, case) in cases.into_iter().enumerate() {
        let label = case.label();
        let run = run_case_with_timeout(case, fuel, timeout_ms);
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
                if let Some(cases) = &mut junit_cases {
                    cases.push(JunitCase {
                        name: label.clone(),
                        failure: None,
                    });
                }
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
                if let Some(cases) = &mut junit_cases {
                    cases.push(JunitCase {
                        name: label.clone(),
                        failure: Some(detail.clone()),
                    });
                }
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
    if let (Some(path), Some(cases)) = (junit, junit_cases) {
        if let Err(error) = write_junit(&path, &cases) {
            eprintln!("qtest could not write JUnit report: {error}");
            failed += 1;
        }
    }
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        eprintln!("{failed} test file(s) failed");
        ExitCode::from(1)
    }
}

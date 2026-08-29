//! Command-line entry point for the `qcoffee` interpreter.

mod cli_diagnostic;

use quickcoffee::{
    CompileLimits, Context, Engine, Error, RestrictedFileModuleLoader, Runtime, Value,
};
use std::{
    collections::BTreeMap,
    env, fmt, fs,
    io::{self, BufRead, IsTerminal, Read, Write},
    process::ExitCode,
};

fn usage() {
    eprintln!(
        "Usage: qcoffee [--fuel N] [--max-source-bytes N] [--max-bytecode-instructions N] [--max-module-graph-modules N] [--max-module-graph-source-bytes N] [--stats] [--json] [-i | -e SOURCE | --check FILE | --dump-bytecode FILE | --fingerprint FILE | --module-root ROOT ENTRY [--fingerprint] | FILE | -] [-- ARG...]\n       qcoffee --interactive\n       qcoffee --quit\n       qcoffee --version"
    );
}

enum ReadSourceError {
    Io(String),
    SourceBytes(usize),
}
impl fmt::Display for ReadSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message) => write!(formatter, "read error: {message}"),
            Self::SourceBytes(limit) => write!(
                formatter,
                "resource error: source exceeds configured UTF-8 byte limit of {limit}"
            ),
        }
    }
}
fn read_limited(reader: impl Read, limit: usize) -> Result<String, ReadSourceError> {
    let mut source = String::new();
    reader
        .take((limit as u64).saturating_add(1))
        .read_to_string(&mut source)
        .map_err(|error| ReadSourceError::Io(error.to_string()))?;
    if source.len() > limit {
        Err(ReadSourceError::SourceBytes(limit))
    } else {
        Ok(source)
    }
}
fn read_source(path: &str, limit: usize) -> Result<String, ReadSourceError> {
    if path == "-" {
        read_limited(io::stdin().lock(), limit)
    } else {
        let file = fs::File::open(path).map_err(|error| ReadSourceError::Io(error.to_string()))?;
        if file
            .metadata()
            .map_err(|error| ReadSourceError::Io(error.to_string()))?
            .len()
            > limit as u64
        {
            return Err(ReadSourceError::SourceBytes(limit));
        }
        read_limited(file, limit)
    }
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
fn json_value(value: &Value) -> String {
    match value.kind() {
        quickcoffee::ValueKind::Nil => "null".to_owned(),
        quickcoffee::ValueKind::Bool => value.as_bool().unwrap().to_string(),
        quickcoffee::ValueKind::Number => {
            let number = value.as_number().unwrap();
            if number.is_finite() {
                number.to_string()
            } else {
                "null".to_owned()
            }
        }
        quickcoffee::ValueKind::Integer => format!(
            "{{\"$quickcoffee\":\"integer\",\"value\":\"{}\"}}",
            value.as_integer().unwrap().to_decimal_string()
        ),
        quickcoffee::ValueKind::Decimal => format!(
            "{{\"$quickcoffee\":\"decimal\",\"value\":\"{}\"}}",
            value.as_decimal().unwrap().to_plain_string()
        ),
        quickcoffee::ValueKind::String => {
            format!("\"{}\"", json_escape(value.as_str().unwrap()))
        }
        quickcoffee::ValueKind::Array => format!(
            "[{}]",
            value
                .as_array()
                .unwrap()
                .iter()
                .map(json_value)
                .collect::<Vec<_>>()
                .join(",")
        ),
        quickcoffee::ValueKind::Map => format!(
            "{{{}}}",
            value
                .as_map()
                .unwrap()
                .iter()
                .map(|(key, value)| { format!("\"{}\":{}", json_escape(key), json_value(value)) })
                .collect::<Vec<_>>()
                .join(",")
        ),
        quickcoffee::ValueKind::Error => {
            let error = value.as_error().unwrap();
            let cause = error.cause().map_or_else(
                || "null".to_owned(),
                |cause| json_value(&Value::Error(std::rc::Rc::new(cause.clone()))),
            );
            format!(
                "{{\"$quickcoffee\":\"error\",\"code\":\"{}\",\"message\":\"{}\",\"data\":{},\"cause\":{}}}",
                json_escape(error.code()),
                json_escape(error.message()),
                json_value(error.data()),
                cause
            )
        }
        quickcoffee::ValueKind::Class => "{\"$quickcoffee\":\"class\"}".to_owned(),
        quickcoffee::ValueKind::Instance => "{\"$quickcoffee\":\"instance\"}".to_owned(),
        quickcoffee::ValueKind::Function => "{\"$quickcoffee\":\"function\"}".to_owned(),
    }
}
fn json_error(error: &Error) -> String {
    let line = error
        .position()
        .map_or_else(|| "null".to_owned(), |position| position.line.to_string());
    let source = error
        .labels()
        .iter()
        .find(|label| label.kind == quickcoffee::DiagnosticLabelKind::Primary)
        .and_then(|label| label.span.source_name.as_deref())
        .map_or_else(String::new, |source_name| {
            format!(",\"source\":\"{}\"", json_escape(source_name))
        });
    let domain = error
        .script_error()
        .map_or_else(String::new, |script_error| {
            let cause = script_error.cause().map_or_else(
                || "null".to_owned(),
                |cause| json_value(&Value::Error(std::rc::Rc::new(cause.clone()))),
            );
            format!(
                ",\"code\":\"{}\",\"data\":{},\"cause\":{}",
                json_escape(script_error.code()),
                json_value(script_error.data()),
                cause
            )
        });
    format!(
        "{{\"ok\":false,\"kind\":\"{}\",\"message\":\"{}\"{domain}{source},\"line\":{line},\"diagnostic\":{}}}",
        error.kind(),
        json_escape(error.message()),
        json_diagnostic(error)
    )
}
fn json_diagnostic(error: &Error) -> String {
    let labels = error
        .labels()
        .iter()
        .map(json_label)
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"version\":1,\"labels\":[{labels}]}}")
}
fn json_label(label: &quickcoffee::DiagnosticLabel) -> String {
    let kind = match label.kind {
        quickcoffee::DiagnosticLabelKind::Primary => "primary",
        quickcoffee::DiagnosticLabelKind::Secondary => "secondary",
    };
    let source = label.span.source_name.as_deref().map_or_else(
        || "null".to_owned(),
        |source| format!("\"{}\"", json_escape(source)),
    );
    let end = label.span.end.map_or_else(
        || "null".to_owned(),
        |position| json_position(position.line, position.column),
    );
    let message = label.message.as_deref().map_or_else(
        || "null".to_owned(),
        |message| format!("\"{}\"", json_escape(message)),
    );
    format!(
        "{{\"kind\":\"{kind}\",\"source\":{source},\"start\":{},\"end\":{end},\"message\":{message}}}",
        json_position(label.span.start.line, label.span.start.column)
    )
}
fn json_position(line: usize, column: Option<usize>) -> String {
    let column = column.map_or_else(|| "null".to_owned(), |column| column.to_string());
    format!("{{\"line\":{line},\"column\":{column}}}")
}
fn empty_json_diagnostic() -> &'static str {
    "{\"version\":1,\"labels\":[]}"
}
fn json_io_error(stage: &str, message: &str) -> String {
    format!(
        "{{\"ok\":false,\"stage\":\"{stage}\",\"kind\":\"io\",\"message\":\"{}\",\"line\":null,\"diagnostic\":{}}}",
        json_escape(message),
        empty_json_diagnostic()
    )
}
fn json_read_error(error: &ReadSourceError) -> String {
    match error {
        ReadSourceError::Io(message) => json_io_error("read", &format!("read error: {message}")),
        ReadSourceError::SourceBytes(limit) => format!(
            "{{\"ok\":false,\"stage\":\"read\",\"kind\":\"resource\",\"limit\":\"source_bytes\",\"message\":\"source exceeds configured UTF-8 byte limit of {limit}\",\"line\":null,\"diagnostic\":{}}}",
            empty_json_diagnostic()
        ),
    }
}
fn render_source_error(error: &Error, source_name: Option<&str>, source: &str) -> String {
    let anonymous_source = source_name.is_none().then_some(source);
    cli_diagnostic::render_error(error, anonymous_source, |requested| {
        (source_name == Some(requested)).then(|| source.to_owned())
    })
}
fn render_module_error(
    error: &Error,
    loader: &cli_diagnostic::RecordingModuleLoader<'_>,
) -> String {
    cli_diagnostic::render_error(error, None, |source_name| loader.source(source_name))
}
fn module_exports_value(exports: &quickcoffee::ModuleExports) -> Value {
    Value::map(
        exports
            .iter()
            .map(|(name, value)| (name.to_owned(), value.clone())),
    )
}
fn repl(
    fuel: u64,
    script_args: Vec<String>,
    stats: bool,
    compile_limits: CompileLimits,
) -> ExitCode {
    let stdin = io::stdin();
    let show_prompt = stdin.is_terminal() && io::stdout().is_terminal();
    let runtime = Runtime::builder().compile_limits(compile_limits).build();
    let mut context = runtime.new_context().with_fuel(fuel);
    context.set_global(
        "argv",
        Value::array(script_args.into_iter().map(Value::from).collect::<Vec<_>>()),
    );
    if show_prompt {
        println!(
            "QuickCoffee {} — :help for commands, :quit to exit",
            env!("CARGO_PKG_VERSION")
        );
    }
    let mut lines = stdin.lock().lines();
    let mut source_index = 0u64;
    let mut sources = BTreeMap::<String, String>::new();
    loop {
        if show_prompt {
            print!("qcoffee> ");
            if io::stdout().flush().is_err() {
                return ExitCode::from(1);
            }
        }
        let Some(line) = (match lines.next() {
            Some(Ok(line)) => Some(line),
            Some(Err(error)) => {
                eprintln!("read error: {error}");
                return ExitCode::from(1);
            }
            None => None,
        }) else {
            break;
        };
        match line.trim() {
            ":quit" | ":exit" => break,
            ":help" => {
                println!(
                    ":help  show this help\n:quit  exit the session\n\nEach non-command input line is one source unit."
                );
            }
            "" => {}
            source => {
                source_index = source_index.saturating_add(1);
                let source_name = format!("<repl:{source_index}>");
                sources.insert(source_name.clone(), source.to_owned());
                let result = context.eval_named(&source_name, source);
                let report_stats = stats
                    && (result.is_ok()
                        || result.as_ref().err().is_some_and(|error| {
                            matches!(
                                error.kind(),
                                quickcoffee::ErrorKind::Runtime | quickcoffee::ErrorKind::Resource
                            )
                        }));
                if report_stats {
                    let execution = context.last_execution();
                    eprintln!(
                        "qcoffee stats: instructions={} fuel_remaining={} name_loads={} name_stores={} calls={} container_ops={} iterator_ops={} exception_ops={} value_allocations={} environment_allocations={} managed_objects_allocated={} managed_bytes_allocated={}",
                        execution.instructions,
                        execution.fuel_remaining,
                        execution.name_loads,
                        execution.name_stores,
                        execution.calls,
                        execution.container_ops,
                        execution.iterator_ops,
                        execution.exception_ops,
                        execution.value_allocations,
                        execution.environment_allocations,
                        execution.managed_objects_allocated,
                        execution.managed_bytes_allocated
                    );
                }
                match result {
                    Ok(value) if !matches!(value, Value::Nil) => println!("{value}"),
                    Ok(_) => {}
                    Err(error) => eprintln!(
                        "{}",
                        cli_diagnostic::render_error(&error, None, |requested| {
                            sources.get(requested).cloned()
                        })
                    ),
                }
            }
        }
    }
    ExitCode::SUCCESS
}
fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let mut fuel = 1_000_000u64;
    let mut fuel_set = false;
    let mut compile_limits = CompileLimits::default();
    let mut compile_limits_set = false;
    let mut source = None;
    let mut source_name = None;
    let mut dump = false;
    let mut fingerprint = false;
    let mut check = false;
    let mut stats = false;
    let mut json = false;
    let mut interactive = false;
    let mut quit = false;
    let mut module_root = None;
    let mut module_entry = None;
    let mut script_args = vec![];
    while let Some(arg) = args.next() {
        if quit {
            eprintln!("--quit cannot be combined with execution options or a source");
            return ExitCode::from(2);
        }
        match arg.as_str() {
            "--" => {
                script_args.extend(args);
                break;
            }
            "--version" => {
                println!("qcoffee {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--help" | "-h" => {
                usage();
                return ExitCode::SUCCESS;
            }
            "--interactive" | "-i" => interactive = true,
            "--fuel" => match args.next().and_then(|s| s.parse().ok()) {
                Some(n) => {
                    fuel = n;
                    fuel_set = true;
                }
                None => {
                    eprintln!("--fuel requires a non-negative integer");
                    return ExitCode::from(2);
                }
            },
            "--max-source-bytes" => match args.next().and_then(|s| s.parse().ok()) {
                Some(limit) => {
                    compile_limits = compile_limits.with_max_source_bytes(limit);
                    compile_limits_set = true;
                }
                None => {
                    eprintln!("--max-source-bytes requires a non-negative integer");
                    return ExitCode::from(2);
                }
            },
            "--max-bytecode-instructions" => match args.next().and_then(|s| s.parse().ok()) {
                Some(limit) => {
                    compile_limits = compile_limits.with_max_bytecode_instructions(limit);
                    compile_limits_set = true;
                }
                None => {
                    eprintln!("--max-bytecode-instructions requires a non-negative integer");
                    return ExitCode::from(2);
                }
            },
            "--max-module-graph-modules" => match args.next().and_then(|s| s.parse().ok()) {
                Some(limit) => {
                    compile_limits = compile_limits.with_max_module_graph_modules(limit);
                    compile_limits_set = true;
                }
                None => {
                    eprintln!("--max-module-graph-modules requires a non-negative integer");
                    return ExitCode::from(2);
                }
            },
            "--max-module-graph-source-bytes" => match args.next().and_then(|s| s.parse().ok()) {
                Some(limit) => {
                    compile_limits = compile_limits.with_max_module_graph_source_bytes(limit);
                    compile_limits_set = true;
                }
                None => {
                    eprintln!("--max-module-graph-source-bytes requires a non-negative integer");
                    return ExitCode::from(2);
                }
            },
            "--stats" => stats = true,
            "--json" => json = true,
            "--quit" => {
                if fuel_set
                    || compile_limits_set
                    || source.is_some()
                    || dump
                    || fingerprint
                    || check
                    || stats
                    || json
                    || interactive
                    || module_root.is_some()
                    || module_entry.is_some()
                {
                    eprintln!("--quit cannot be combined with execution options or a source");
                    return ExitCode::from(2);
                }
                quit = true;
            }
            "-e" => match args.next() {
                Some(s)
                    if source.is_none()
                        && module_root.is_none()
                        && !dump
                        && !check
                        && !fingerprint =>
                {
                    source = Some(s)
                }
                Some(_) => {
                    eprintln!("-e cannot be combined with another source or execution mode");
                    return ExitCode::from(2);
                }
                None => {
                    eprintln!("-e requires source text");
                    return ExitCode::from(2);
                }
            },
            "--dump-bytecode" => {
                if json {
                    eprintln!("--json cannot be combined with --dump-bytecode");
                    return ExitCode::from(2);
                }
                if source.is_some()
                    || module_root.is_some()
                    || dump
                    || check
                    || fingerprint
                    || stats
                {
                    eprintln!(
                        "-e, --check, --dump-bytecode, --fingerprint, --json, and --stats are execution-mode alternatives"
                    );
                    return ExitCode::from(2);
                }
                dump = true;
                match args.next() {
                    Some(path) if path == "-" || !path.starts_with('-') => {
                        match read_source(&path, compile_limits.max_source_bytes()) {
                            Ok(text) => {
                                source = Some(text);
                                source_name = Some(path);
                            }
                            Err(error) => {
                                eprintln!("{error}");
                                return ExitCode::from(1);
                            }
                        }
                    }
                    Some(_) => {
                        eprintln!("--dump-bytecode requires a file");
                        return ExitCode::from(2);
                    }
                    None => {
                        eprintln!("--dump-bytecode requires a file");
                        return ExitCode::from(2);
                    }
                }
            }
            "--check" => {
                if json {
                    eprintln!("--json cannot be combined with --check");
                    return ExitCode::from(2);
                }
                if source.is_some()
                    || module_root.is_some()
                    || dump
                    || check
                    || fingerprint
                    || stats
                {
                    eprintln!(
                        "-e, --check, --dump-bytecode, --fingerprint, --json, and --stats are execution-mode alternatives"
                    );
                    return ExitCode::from(2);
                }
                check = true;
                match args.next() {
                    Some(path) => match read_source(&path, compile_limits.max_source_bytes()) {
                        Ok(text) => {
                            source = Some(text);
                            source_name = Some(path);
                        }
                        Err(error) => {
                            if json {
                                println!("{}", json_read_error(&error));
                            } else {
                                eprintln!("{error}");
                            }
                            return ExitCode::from(1);
                        }
                    },
                    None => {
                        eprintln!("--check requires a file");
                        return ExitCode::from(2);
                    }
                }
            }
            "--fingerprint" => {
                if json {
                    eprintln!("--json cannot be combined with --fingerprint");
                    return ExitCode::from(2);
                }
                if source.is_some() || dump || check || fingerprint || stats {
                    eprintln!(
                        "-e, --check, --dump-bytecode, --fingerprint, --json, and --stats are execution-mode alternatives"
                    );
                    return ExitCode::from(2);
                }
                fingerprint = true;
            }
            "--module-root" => {
                if source.is_some() || module_root.is_some() || dump || check || interactive {
                    eprintln!(
                        "--module-root cannot be combined with another source or execution mode"
                    );
                    return ExitCode::from(2);
                }
                match args.next() {
                    Some(root) => module_root = Some(root),
                    None => {
                        eprintln!("--module-root requires a directory");
                        return ExitCode::from(2);
                    }
                }
            }
            "-" if source.is_none() && module_root.is_none() => {
                match read_source("-", compile_limits.max_source_bytes()) {
                    Ok(text) => {
                        source = Some(text);
                        source_name = Some("-".to_owned());
                    }
                    Err(error) => {
                        if json {
                            println!("{}", json_read_error(&error));
                        } else {
                            eprintln!("{error}");
                        }
                        return ExitCode::from(1);
                    }
                }
            }
            path if !path.starts_with('-') && module_root.is_some() && module_entry.is_none() => {
                module_entry = Some(path.to_owned());
            }
            path if !path.starts_with('-') && source.is_none() && module_root.is_none() => {
                match read_source(path, compile_limits.max_source_bytes()) {
                    Ok(text) => {
                        source = Some(text);
                        source_name = Some(path.to_owned());
                    }
                    Err(error) => {
                        if json {
                            println!("{}", json_read_error(&error));
                        } else {
                            eprintln!("{error}");
                        }
                        return ExitCode::from(1);
                    }
                }
            }
            _ => {
                usage();
                return ExitCode::from(2);
            }
        }
    }
    if quit {
        let _context = Context::new();
        return ExitCode::SUCCESS;
    }
    if json && (dump || check || fingerprint) {
        eprintln!(
            "--check, --dump-bytecode, --fingerprint, and --json are execution-mode alternatives"
        );
        return ExitCode::from(2);
    }
    if interactive {
        if source.is_some() || module_root.is_some() || check || dump || fingerprint || json {
            eprintln!(
                "--interactive cannot be combined with a source, --check, --dump-bytecode, --fingerprint, or --json"
            );
            return ExitCode::from(2);
        }
        return repl(fuel, script_args, stats, compile_limits);
    }
    if let Some(root) = module_root {
        let Some(entry) = module_entry else {
            eprintln!("--module-root requires an entry module");
            return ExitCode::from(2);
        };
        if stats && (dump || check || fingerprint) {
            eprintln!(
                "--check, --dump-bytecode, --fingerprint, and --stats are execution-mode alternatives"
            );
            return ExitCode::from(2);
        }
        let loader = match RestrictedFileModuleLoader::new(&root)
            .map(|loader| loader.with_max_source_bytes(compile_limits.max_source_bytes()))
        {
            Ok(loader) => loader,
            Err(error) => {
                if json {
                    println!("{}", json_error(&error));
                } else {
                    eprintln!("{}", cli_diagnostic::render_error(&error, None, |_| None));
                }
                return ExitCode::from(1);
            }
        };
        let diagnostic_loader = cli_diagnostic::RecordingModuleLoader::new(&loader);
        let source = match loader.load_entry(&entry) {
            Ok(source) => {
                diagnostic_loader.record(&source);
                source
            }
            Err(error) => {
                if json {
                    println!("{}", json_error(&error));
                } else {
                    eprintln!("{}", render_module_error(&error, &diagnostic_loader));
                }
                return ExitCode::from(1);
            }
        };
        let runtime = Runtime::builder().compile_limits(compile_limits).build();
        let module = match runtime.compile_module(source.name(), source.source()) {
            Ok(module) => module,
            Err(error) => {
                if json {
                    println!("{}", json_error(&error));
                } else {
                    eprintln!("{}", render_module_error(&error, &diagnostic_loader));
                }
                return ExitCode::from(1);
            }
        };
        if fingerprint {
            return match runtime
                .engine()
                .fingerprint_module_graph(&module, &diagnostic_loader)
            {
                Ok(fingerprint) => {
                    println!("{fingerprint:016x}");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{}", render_module_error(&error, &diagnostic_loader));
                    ExitCode::from(1)
                }
            };
        }
        let mut context = runtime.new_context().with_fuel(fuel);
        context.set_global(
            "argv",
            Value::array(script_args.into_iter().map(Value::from).collect::<Vec<_>>()),
        );
        let result = context.run_module(&module, &diagnostic_loader);
        if stats {
            let execution = context.last_execution();
            eprintln!(
                "qcoffee stats: instructions={} fuel_remaining={} name_loads={} name_stores={} calls={} container_ops={} iterator_ops={} exception_ops={} value_allocations={} environment_allocations={} managed_objects_allocated={} managed_bytes_allocated={}",
                execution.instructions,
                execution.fuel_remaining,
                execution.name_loads,
                execution.name_stores,
                execution.calls,
                execution.container_ops,
                execution.iterator_ops,
                execution.exception_ops,
                execution.value_allocations,
                execution.environment_allocations,
                execution.managed_objects_allocated,
                execution.managed_bytes_allocated
            );
        }
        return match result {
            Ok(exports) => {
                let exports = module_exports_value(&exports);
                if json {
                    println!("{{\"ok\":true,\"exports\":{}}}", json_value(&exports));
                } else {
                    println!("{exports}");
                }
                ExitCode::SUCCESS
            }
            Err(error) => {
                if json {
                    println!("{}", json_error(&error));
                } else {
                    eprintln!("{}", render_module_error(&error, &diagnostic_loader));
                }
                ExitCode::from(1)
            }
        };
    }
    let Some(source) = source else {
        if fingerprint {
            eprintln!("--fingerprint requires a file or --module-root ROOT ENTRY");
            return ExitCode::from(2);
        }
        usage();
        return ExitCode::from(2);
    };
    if stats && (dump || check || fingerprint) {
        eprintln!(
            "--check, --dump-bytecode, --fingerprint, and --stats are execution-mode alternatives"
        );
        return ExitCode::from(2);
    }
    let engine = Engine::new().with_compile_limits(compile_limits);
    if check {
        let checked = match source_name.as_deref() {
            Some(source_name) => engine.check_program_named(source_name, &source),
            None => engine.check_program(&source),
        };
        return match checked {
            Ok(()) => ExitCode::SUCCESS,
            Err(errors) => {
                for error in errors {
                    eprintln!(
                        "{}",
                        render_source_error(&error, source_name.as_deref(), &source)
                    );
                }
                ExitCode::from(1)
            }
        };
    }
    let compiled = match source_name.as_deref() {
        Some(source_name) => engine.compile_program_named(source_name, &source),
        None => engine.compile_program(&source),
    };
    let program = match compiled {
        Ok(program) => program,
        Err(e) => {
            if json {
                println!("{}", json_error(&e));
            } else {
                eprintln!(
                    "{}",
                    render_source_error(&e, source_name.as_deref(), &source)
                );
            }
            return ExitCode::from(1);
        }
    };
    if dump {
        print!("{}", program.disassemble());
        return ExitCode::SUCCESS;
    }
    if fingerprint {
        println!("{:016x}", program.fingerprint());
        return ExitCode::SUCCESS;
    }
    let runtime = Runtime::builder().compile_limits(compile_limits).build();
    let mut context = runtime.new_context().with_fuel(fuel);
    context.set_global(
        "argv",
        Value::array(script_args.into_iter().map(Value::from).collect::<Vec<_>>()),
    );
    let result = context.run_program(&program);
    if stats {
        let execution = context.last_execution();
        eprintln!(
            "qcoffee stats: instructions={} fuel_remaining={} name_loads={} name_stores={} calls={} container_ops={} iterator_ops={} exception_ops={} value_allocations={} environment_allocations={} managed_objects_allocated={} managed_bytes_allocated={}",
            execution.instructions,
            execution.fuel_remaining,
            execution.name_loads,
            execution.name_stores,
            execution.calls,
            execution.container_ops,
            execution.iterator_ops,
            execution.exception_ops,
            execution.value_allocations,
            execution.environment_allocations,
            execution.managed_objects_allocated,
            execution.managed_bytes_allocated
        );
    }
    match result {
        Ok(value) => {
            if json {
                println!("{{\"ok\":true,\"value\":{}}}", json_value(&value));
            } else if !matches!(value, quickcoffee::Value::Nil) {
                println!("{value}")
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            if json {
                println!("{}", json_error(&e));
            } else {
                eprintln!(
                    "{}",
                    render_source_error(&e, source_name.as_deref(), &source)
                );
            }
            ExitCode::from(1)
        }
    }
}

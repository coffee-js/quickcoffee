//! Command-line entry point for the `qcoffee` interpreter.

use quickcoffee::{Context, Engine, Error, Value};
use std::{
    env, fs,
    io::{self, BufRead, IsTerminal, Write},
    process::ExitCode,
};

fn usage() {
    eprintln!(
        "Usage: qcoffee [--fuel N] [--stats] [--json] [-i | -e SOURCE | --check FILE | --dump-bytecode FILE | --fingerprint FILE | FILE | -] [-- ARG...]\n       qcoffee --interactive\n       qcoffee --version"
    );
}
fn read_source(path: &str) -> Result<String, String> {
    if path == "-" {
        io::read_to_string(io::stdin()).map_err(|error| format!("read error: {error}"))
    } else {
        fs::read_to_string(path).map_err(|error| format!("read error: {error}"))
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
        quickcoffee::ValueKind::Function => "{\"$quickcoffee\":\"function\"}".to_owned(),
    }
}
fn json_error(error: &Error) -> String {
    let line = error
        .position()
        .map_or_else(|| "null".to_owned(), |position| position.line.to_string());
    format!(
        "{{\"ok\":false,\"kind\":\"{}\",\"message\":\"{}\",\"line\":{line}}}",
        error.kind(),
        json_escape(error.message())
    )
}
fn json_io_error(stage: &str, message: &str) -> String {
    format!(
        "{{\"ok\":false,\"stage\":\"{stage}\",\"kind\":\"io\",\"message\":\"{}\",\"line\":null}}",
        json_escape(message)
    )
}
fn repl(fuel: u64, script_args: Vec<String>, stats: bool) -> ExitCode {
    let stdin = io::stdin();
    let show_prompt = stdin.is_terminal() && io::stdout().is_terminal();
    let mut context = Context::new().with_fuel(fuel);
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
                println!(":help  show this help\n:quit  exit the session");
            }
            "" => {}
            source => {
                let result = context.eval(source);
                let report_stats = stats
                    && (result.is_ok()
                        || result
                            .as_ref()
                            .err()
                            .is_some_and(|error| error.kind() == quickcoffee::ErrorKind::Runtime));
                if report_stats {
                    let execution = context.last_execution();
                    eprintln!(
                        "qcoffee stats: instructions={} fuel_remaining={}",
                        execution.instructions, execution.fuel_remaining
                    );
                }
                match result {
                    Ok(value) if !matches!(value, Value::Nil) => println!("{value}"),
                    Ok(_) => {}
                    Err(error) => eprintln!("{error}"),
                }
            }
        }
    }
    ExitCode::SUCCESS
}
fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let mut fuel = 1_000_000u64;
    let mut source = None;
    let mut dump = false;
    let mut fingerprint = false;
    let mut check = false;
    let mut stats = false;
    let mut json = false;
    let mut interactive = false;
    let mut script_args = vec![];
    while let Some(arg) = args.next() {
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
                Some(n) => fuel = n,
                None => {
                    eprintln!("--fuel requires a non-negative integer");
                    return ExitCode::from(2);
                }
            },
            "--stats" => stats = true,
            "--json" => json = true,
            "-e" => match args.next() {
                Some(s) if source.is_none() => source = Some(s),
                Some(_) => {
                    eprintln!("only one source input is allowed");
                    return ExitCode::from(2);
                }
                None => {
                    eprintln!("-e requires source text");
                    return ExitCode::from(2);
                }
            },
            "--dump-bytecode" => {
                if source.is_some() || dump || check || fingerprint || stats || json {
                    eprintln!(
                        "--check, --dump-bytecode, --fingerprint, and --stats are execution-mode alternatives"
                    );
                    return ExitCode::from(2);
                }
                dump = true;
                match args.next() {
                    Some(path) if path == "-" || !path.starts_with('-') => match read_source(&path)
                    {
                        Ok(text) => source = Some(text),
                        Err(error) => {
                            eprintln!("{error}");
                            return ExitCode::from(1);
                        }
                    },
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
                if source.is_some() || dump || check || fingerprint || stats || json {
                    eprintln!(
                        "--check, --dump-bytecode, --fingerprint, and --stats are execution-mode alternatives"
                    );
                    return ExitCode::from(2);
                }
                check = true;
                match args.next() {
                    Some(path) => match read_source(&path) {
                        Ok(text) => source = Some(text),
                        Err(error) => {
                            if json {
                                println!("{}", json_io_error("read", &error));
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
                if source.is_some() || dump || check || fingerprint || stats || json {
                    eprintln!(
                        "--check, --dump-bytecode, --fingerprint, and --stats are execution-mode alternatives"
                    );
                    return ExitCode::from(2);
                }
                fingerprint = true;
                match args.next() {
                    Some(path) if path == "-" || !path.starts_with('-') => match read_source(&path)
                    {
                        Ok(text) => source = Some(text),
                        Err(error) => {
                            eprintln!("{error}");
                            return ExitCode::from(1);
                        }
                    },
                    Some(_) => {
                        eprintln!("--fingerprint requires a file");
                        return ExitCode::from(2);
                    }
                    None => {
                        eprintln!("--fingerprint requires a file");
                        return ExitCode::from(2);
                    }
                }
            }
            "-" if source.is_none() => match read_source("-") {
                Ok(text) => source = Some(text),
                Err(error) => {
                    if json {
                        println!("{}", json_io_error("read", &error));
                    } else {
                        eprintln!("{error}");
                    }
                    return ExitCode::from(1);
                }
            },
            path if !path.starts_with('-') && source.is_none() => match read_source(path) {
                Ok(text) => source = Some(text),
                Err(error) => {
                    if json {
                        println!("{}", json_io_error("read", &error));
                    } else {
                        eprintln!("{error}");
                    }
                    return ExitCode::from(1);
                }
            },
            _ => {
                usage();
                return ExitCode::from(2);
            }
        }
    }
    if interactive {
        if source.is_some() || check || dump || fingerprint || json {
            eprintln!(
                "--interactive cannot be combined with a source, --check, --dump-bytecode, --fingerprint, or --json"
            );
            return ExitCode::from(2);
        }
        return repl(fuel, script_args, stats);
    }
    let Some(source) = source else {
        usage();
        return ExitCode::from(2);
    };
    if stats && (dump || check || fingerprint) {
        eprintln!(
            "--check, --dump-bytecode, --fingerprint, and --stats are execution-mode alternatives"
        );
        return ExitCode::from(2);
    }
    let engine = Engine::new();
    let chunk = match engine.compile(&source) {
        Ok(c) => c,
        Err(e) => {
            if json {
                println!("{}", json_error(&e));
            } else {
                eprintln!("{e}");
            }
            return ExitCode::from(1);
        }
    };
    if dump {
        print!("{}", chunk.disassemble());
        return ExitCode::SUCCESS;
    }
    if check {
        return ExitCode::SUCCESS;
    }
    if fingerprint {
        println!("{:016x}", chunk.fingerprint());
        return ExitCode::SUCCESS;
    }
    let mut context = Context::new().with_fuel(fuel);
    context.set_global(
        "argv",
        Value::array(script_args.into_iter().map(Value::from).collect::<Vec<_>>()),
    );
    let result = context.run(chunk);
    if stats {
        let execution = context.last_execution();
        eprintln!(
            "qcoffee stats: instructions={} fuel_remaining={}",
            execution.instructions, execution.fuel_remaining
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
                eprintln!("{e}");
            }
            ExitCode::from(1)
        }
    }
}

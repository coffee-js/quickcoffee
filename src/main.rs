use quickcoffee::{Context, Engine, Value};
use std::{env, fs, io, process::ExitCode};

fn usage() {
    eprintln!(
        "Usage: qcoffee [--fuel N] [-e SOURCE | --check FILE | --dump-bytecode FILE | FILE | -] [-- ARG...]\n       qcoffee --version"
    );
}
fn read_source(path: &str) -> Result<String, String> {
    if path == "-" {
        io::read_to_string(io::stdin()).map_err(|error| format!("read error: {error}"))
    } else {
        fs::read_to_string(path).map_err(|error| format!("read error: {error}"))
    }
}
fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let mut fuel = 1_000_000u64;
    let mut source = None;
    let mut dump = false;
    let mut check = false;
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
            "--fuel" => match args.next().and_then(|s| s.parse().ok()) {
                Some(n) => fuel = n,
                None => {
                    eprintln!("--fuel requires a non-negative integer");
                    return ExitCode::from(2);
                }
            },
            "-e" => match args.next() {
                Some(s) => source = Some(s),
                None => {
                    eprintln!("-e requires source text");
                    return ExitCode::from(2);
                }
            },
            "--dump-bytecode" => {
                if check {
                    eprintln!("--check and --dump-bytecode cannot be combined");
                    return ExitCode::from(2);
                }
                dump = true;
                match args.next() {
                    Some(path) => match read_source(&path) {
                        Ok(text) => source = Some(text),
                        Err(error) => {
                            eprintln!("{error}");
                            return ExitCode::from(1);
                        }
                    },
                    None => {
                        eprintln!("--dump-bytecode requires a file");
                        return ExitCode::from(2);
                    }
                }
            }
            "--check" => {
                if dump {
                    eprintln!("--check and --dump-bytecode cannot be combined");
                    return ExitCode::from(2);
                }
                check = true;
                match args.next() {
                    Some(path) => match read_source(&path) {
                        Ok(text) => source = Some(text),
                        Err(error) => {
                            eprintln!("{error}");
                            return ExitCode::from(1);
                        }
                    },
                    None => {
                        eprintln!("--check requires a file");
                        return ExitCode::from(2);
                    }
                }
            }
            "-" if source.is_none() => match read_source("-") {
                Ok(text) => source = Some(text),
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::from(1);
                }
            },
            path if !path.starts_with('-') && source.is_none() => match read_source(path) {
                Ok(text) => source = Some(text),
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::from(1);
                }
            },
            _ => {
                usage();
                return ExitCode::from(2);
            }
        }
    }
    let Some(source) = source else {
        usage();
        return ExitCode::from(2);
    };
    let engine = Engine::new();
    let chunk = match engine.compile(&source) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
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
    let mut context = Context::new().with_fuel(fuel);
    context.set_global(
        "argv",
        Value::array(script_args.into_iter().map(Value::from).collect::<Vec<_>>()),
    );
    match context.run(chunk) {
        Ok(value) => {
            if !matches!(value, quickcoffee::Value::Nil) {
                println!("{value}")
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

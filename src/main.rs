use quickcoffee::{Context, Engine, Value};
use std::{
    env, fs,
    io::{self, BufRead, IsTerminal, Write},
    process::ExitCode,
};

fn usage() {
    eprintln!(
        "Usage: qcoffee [--fuel N] [--stats] [-i | -e SOURCE | --check FILE | --dump-bytecode FILE | FILE | -] [-- ARG...]\n       qcoffee --interactive\n       qcoffee --version"
    );
}
fn read_source(path: &str) -> Result<String, String> {
    if path == "-" {
        io::read_to_string(io::stdin()).map_err(|error| format!("read error: {error}"))
    } else {
        fs::read_to_string(path).map_err(|error| format!("read error: {error}"))
    }
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
                if stats {
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
    let mut check = false;
    let mut stats = false;
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
            "-e" => match args.next() {
                Some(s) => source = Some(s),
                None => {
                    eprintln!("-e requires source text");
                    return ExitCode::from(2);
                }
            },
            "--dump-bytecode" => {
                if check || stats {
                    eprintln!(
                        "--check, --dump-bytecode, and --stats are execution-mode alternatives"
                    );
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
                if dump || stats {
                    eprintln!(
                        "--check, --dump-bytecode, and --stats are execution-mode alternatives"
                    );
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
    if interactive {
        if source.is_some() || check || dump {
            eprintln!(
                "--interactive cannot be combined with a source, --check, or --dump-bytecode"
            );
            return ExitCode::from(2);
        }
        return repl(fuel, script_args, stats);
    }
    let Some(source) = source else {
        usage();
        return ExitCode::from(2);
    };
    if stats && (dump || check) {
        eprintln!("--check, --dump-bytecode, and --stats are execution-mode alternatives");
        return ExitCode::from(2);
    }
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

//! Literate-programming renderer and checker for QuickCoffee sources.

use quickcoffee::{Context, Engine, Value};
use std::{env, fs, path::PathBuf, process::ExitCode};

fn usage() {
    eprintln!("Usage: qdocco [--check | --markdown] FILE [-o OUTPUT]\n       qdocco --version");
}
fn escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn render(source: &str, result: &str) -> String {
    let mut prose = String::new();
    let mut code = String::new();
    for line in source.lines() {
        if let Some(text) = line.trim_start().strip_prefix("##") {
            prose.push_str(&format!("<p>{}</p>\n", escape(text.trim())))
        } else {
            code.push_str(line);
            code.push('\n');
        }
    }
    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>QuickCoffee document</title><style>body{{font:16px system-ui;display:grid;grid-template-columns:1fr 1fr;gap:2rem;margin:2rem}}pre{{background:#f5f5f5;padding:1rem;white-space:pre-wrap}}footer{{grid-column:1/-1}}</style><main><h1>Notes</h1>{prose}</main><main><h1>Code</h1><pre><code>{}</code></pre></main><footer>Final value: <code>{}</code></footer>",
        escape(&code),
        escape(result)
    )
}
fn render_markdown(source: &str, result: &str) -> String {
    let mut prose = String::new();
    let mut code = String::new();
    for line in source.lines() {
        if let Some(text) = line.trim_start().strip_prefix("##") {
            let text = text.trim();
            if !text.is_empty() {
                prose.push_str(text);
                prose.push('\n');
            }
        } else {
            code.push_str(line);
            code.push('\n');
        }
    }
    format!(
        "# QuickCoffee document\n\n## Notes\n\n{prose}\n## Code\n\n````quickcoffee\n{code}````\n\n## Final value\n\n`{result}`\n"
    )
}
fn main() -> ExitCode {
    let mut check = false;
    let mut markdown = false;
    let mut input = None;
    let mut output = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" => {
                println!("qdocco {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--check" => check = true,
            "--markdown" => markdown = true,
            "-o" => match args.next() {
                Some(x) => output = Some(PathBuf::from(x)),
                None => {
                    usage();
                    return ExitCode::from(2);
                }
            },
            x if !x.starts_with('-') && input.is_none() => input = Some(PathBuf::from(x)),
            _ => {
                usage();
                return ExitCode::from(2);
            }
        }
    }
    let Some(input) = input else {
        usage();
        return ExitCode::from(2);
    };
    if check && markdown {
        eprintln!("--check and --markdown are mutually exclusive");
        return ExitCode::from(2);
    }
    let source = match fs::read_to_string(&input) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("read error: {e}");
            return ExitCode::from(1);
        }
    };
    let chunk = match Engine::new().compile(&source) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let result = match Context::new().run(chunk) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    if check && !matches!(&result, Value::Bool(true)) {
        eprintln!("qdocco check failed: final value was {result}, expected true");
        return ExitCode::from(1);
    }
    if !check {
        let destination =
            output.unwrap_or_else(|| input.with_extension(if markdown { "md" } else { "html" }));
        let document = if markdown {
            render_markdown(&source, &result.to_string())
        } else {
            render(&source, &result.to_string())
        };
        if let Err(e) = fs::write(&destination, document) {
            eprintln!("write error: {e}");
            return ExitCode::from(1);
        }
        println!("wrote {}", destination.display());
    }
    ExitCode::SUCCESS
}

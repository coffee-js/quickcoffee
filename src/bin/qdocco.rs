//! Literate-programming renderer and checker for QuickCoffee sources.

use quickcoffee::{Context, Engine, Value};
use std::{
    env,
    fmt::Write as _,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

fn usage() {
    eprintln!("Usage: qdocco [--check | --markdown] FILE [-o OUTPUT]\n       qdocco --version");
}
fn same_path(left: &PathBuf, right: &PathBuf) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}
fn write_output(destination: &Path, document: &str) -> io::Result<()> {
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("qdocco-output");
    let temporary = destination.with_file_name(format!(
        ".{file_name}.quickcoffee-{}.tmp",
        std::process::id()
    ));
    let mut created = false;
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        created = true;
        file.write_all(document.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, destination)
    })();
    if result.is_err() && created {
        let _ = fs::remove_file(&temporary);
    }
    result
}
fn escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn prose_text(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let text = trimmed.strip_prefix("##")?;
    (!text.starts_with('#')).then_some(text)
}
fn split_source(source: &str) -> (String, String) {
    let mut prose = String::new();
    let mut code = String::new();
    let mut in_block_comment = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if in_block_comment {
            code.push_str(line);
            code.push('\n');
            if trimmed.contains("###") {
                in_block_comment = false;
            }
        } else if let Some(after_marker) = trimmed.strip_prefix("###") {
            code.push_str(line);
            code.push('\n');
            in_block_comment = !after_marker.contains("###");
        } else if let Some(text) = prose_text(line) {
            prose.push_str(text.trim());
            prose.push('\n');
        } else {
            code.push_str(line);
            code.push('\n');
        }
    }
    (prose, code)
}
fn render(source: &str, result: &str) -> String {
    let (prose_text, code) = split_source(source);
    let prose = prose_text.lines().fold(String::new(), |mut prose, line| {
        writeln!(prose, "<p>{}</p>", escape(line)).expect("writing to a string cannot fail");
        prose
    });
    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>QuickCoffee document</title><style>body{{font:16px system-ui;display:grid;grid-template-columns:1fr 1fr;gap:2rem;margin:2rem}}pre{{background:#f5f5f5;padding:1rem;white-space:pre-wrap}}footer{{grid-column:1/-1}}</style><main><h1>Notes</h1>{prose}</main><main><h1>Code</h1><pre><code>{}</code></pre></main><footer>Final value: <code>{}</code></footer>",
        escape(&code),
        escape(result)
    )
}
fn render_markdown(source: &str, result: &str) -> String {
    let (prose, code) = split_source(source);
    let fence = markdown_fence(source);
    format!(
        "# QuickCoffee document\n\n## Notes\n\n{prose}\n## Code\n\n{fence}quickcoffee\n{code}{fence}\n\n## Final value\n\n`{result}`\n"
    )
}
fn markdown_fence(source: &str) -> String {
    let mut longest = 0;
    let mut current = 0;
    for character in source.chars() {
        if character == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    "`".repeat((longest + 1).max(4))
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
        if same_path(&input, &destination) {
            eprintln!("output path must differ from the input source");
            return ExitCode::from(2);
        }
        let document = if markdown {
            render_markdown(&source, &result.to_string())
        } else {
            render(&source, &result.to_string())
        };
        if let Err(e) = write_output(&destination, &document) {
            eprintln!("write error: {e}");
            return ExitCode::from(1);
        }
        println!("wrote {}", destination.display());
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::write_output;
    use std::{fs, path::PathBuf};

    #[test]
    fn output_replacement_is_exclusive_and_cleans_up_on_collision() {
        let directory =
            std::env::temp_dir().join(format!("quickcoffee-qdocco-output-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("temporary directory");
        let destination = directory.join("document.html");
        fs::write(&destination, "old").expect("seed output");
        let temporary = PathBuf::from(format!(
            ".document.html.quickcoffee-{}.tmp",
            std::process::id()
        ));
        let temporary = directory.join(temporary);
        fs::write(&temporary, "reserved").expect("reserve temporary output");

        assert!(write_output(&destination, "new").is_err());
        assert_eq!(fs::read_to_string(&destination).expect("old output"), "old");
        assert_eq!(
            fs::read_to_string(&temporary).expect("reserved temporary output"),
            "reserved"
        );

        fs::remove_file(&temporary).expect("release temporary output");
        write_output(&destination, "new").expect("replace output");
        assert_eq!(fs::read_to_string(&destination).expect("new output"), "new");
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }
}

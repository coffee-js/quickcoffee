//! Literate-programming renderer and checker for QuickCoffee sources.

use quickcoffee::{Context, Value};
use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

const HTML_TEMPLATE_VERSION: &str = "quickcoffee.qdocco.html.v1";
const MARKDOWN_TEMPLATE_VERSION: &str = "quickcoffee.qdocco.markdown.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WriteOutcome {
    Written,
    Unchanged,
}

fn usage() {
    eprintln!(
        "Usage: qdocco [--check | --markdown] [--incremental] DOCUMENT.litcoffee [-o OUTPUT]\n       qdocco --help | --version"
    );
}
fn same_path(left: &PathBuf, right: &PathBuf) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}
fn write_output(destination: &Path, document: &str, incremental: bool) -> io::Result<WriteOutcome> {
    if incremental {
        match fs::read(destination) {
            Ok(existing) if existing == document.as_bytes() => {
                return Ok(WriteOutcome::Unchanged);
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    let file_name = destination
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("qdocco-output"));
    let mut temporary_name = OsString::from(".");
    temporary_name.push(file_name);
    temporary_name.push(format!(".quickcoffee-{}.tmp", std::process::id()));
    let temporary = destination.with_file_name(temporary_name);
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
        #[cfg(windows)]
        if destination.exists() {
            fs::remove_file(destination)?;
        }
        fs::rename(&temporary, destination)?;
        #[cfg(unix)]
        {
            let parent = destination
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            fs::File::open(parent)?.sync_all()?;
        }
        Ok(WriteOutcome::Written)
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
fn literate_code_line(line: &str) -> Option<&str> {
    line.strip_prefix("    ")
        .or_else(|| line.strip_prefix('\t'))
}
fn split_source(source: &str) -> (String, String) {
    let mut prose = String::new();
    let mut code = String::new();
    let mut in_code = false;
    let mut code_may_start = true;
    for line in source.lines() {
        if line.trim().is_empty() {
            prose.push('\n');
            code.push('\n');
            code_may_start = true;
            continue;
        }
        if let Some(code_line) = literate_code_line(line) {
            if in_code || code_may_start {
                code.push_str(code_line);
                code.push('\n');
                in_code = true;
                code_may_start = false;
                continue;
            }
        }
        prose.push_str(line);
        prose.push('\n');
        in_code = false;
        code_may_start = false;
    }
    (trim_section(&prose), trim_section(&code))
}
fn trim_section(section: &str) -> String {
    let section = section.trim_matches('\n');
    if section.is_empty() {
        String::new()
    } else {
        format!("{section}\n")
    }
}
fn normalize_code_span(source: &str) -> String {
    let normalized = source
        .split([' ', '\t', '\n', '\r'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() && !source.is_empty() {
        " ".to_owned()
    } else {
        normalized
    }
}
fn render_inline_html(source: &str) -> String {
    let mut rendered = String::new();
    let mut offset = 0;
    while let Some(relative_start) = source[offset..].find('`') {
        let start = offset + relative_start;
        rendered.push_str(&escape(&source[offset..start]));
        let ticks = source[start..]
            .bytes()
            .take_while(|byte| *byte == b'`')
            .count();
        let content_start = start + ticks;
        let mut search = content_start;
        let closing = loop {
            let Some(relative_tick) = source[search..].find('`') else {
                break None;
            };
            let candidate = search + relative_tick;
            let candidate_ticks = source[candidate..]
                .bytes()
                .take_while(|byte| *byte == b'`')
                .count();
            if candidate_ticks == ticks {
                break Some(candidate);
            }
            search = candidate + candidate_ticks;
        };
        let Some(closing) = closing else {
            rendered.push_str(&escape(&source[start..]));
            return rendered;
        };
        rendered.push_str("<code>");
        rendered.push_str(&escape(&normalize_code_span(
            &source[content_start..closing],
        )));
        rendered.push_str("</code>");
        offset = closing + ticks;
    }
    rendered.push_str(&escape(&source[offset..]));
    rendered
}
fn render_prose_html(source: &str) -> String {
    source
        .split("\n\n")
        .filter_map(|paragraph| {
            let paragraph = paragraph.trim();
            if paragraph.is_empty() {
                return None;
            }
            let (tag, text) = if let Some(text) = paragraph.strip_prefix("### ") {
                ("h3", text)
            } else if let Some(text) = paragraph.strip_prefix("## ") {
                ("h2", text)
            } else if let Some(text) = paragraph.strip_prefix("# ") {
                ("h1", text)
            } else {
                ("p", paragraph)
            };
            Some(format!(
                "<{tag}>{}</{tag}>",
                render_inline_html(&text.replace('\n', " "))
            ))
        })
        .collect()
}
fn render(source: &str, result: &str) -> String {
    let (prose_text, code) = split_source(source);
    let prose = render_prose_html(&prose_text);
    format!(
        "<!doctype html><meta charset=\"utf-8\"><meta name=\"generator\" content=\"{HTML_TEMPLATE_VERSION}\"><title>QuickCoffee document</title><style>body{{font:16px system-ui;display:grid;grid-template-columns:1fr 1fr;gap:2rem;margin:2rem}}pre{{background:#f5f5f5;padding:1rem;white-space:pre-wrap}}footer{{grid-column:1/-1}}</style><main><h1>Notes</h1>{prose}</main><main><h1>Code</h1><pre><code>{}</code></pre></main><footer>Final value: <code>{}</code></footer>",
        escape(&code),
        escape(result)
    )
}
fn render_markdown(source: &str, result: &str) -> String {
    let (prose, code) = split_source(source);
    let fence = markdown_fence(source);
    format!(
        "<!-- {MARKDOWN_TEMPLATE_VERSION} -->\n\n# QuickCoffee document\n\n## Notes\n\n{prose}\n## Code\n\n{fence}coffee\n{code}{fence}\n\n## Final value\n\n`{result}`\n"
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
    let mut incremental = false;
    let mut input = None;
    let mut output = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" => {
                println!("qdocco {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--help" | "-h" => {
                usage();
                return ExitCode::SUCCESS;
            }
            "--check" => check = true,
            "--markdown" => markdown = true,
            "--incremental" => incremental = true,
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
    if input.extension().and_then(|extension| extension.to_str()) != Some("litcoffee") {
        eprintln!("qdocco expects a .litcoffee document");
        return ExitCode::from(2);
    }
    if check && (markdown || incremental) {
        eprintln!("--check cannot be combined with --markdown or --incremental");
        return ExitCode::from(2);
    }
    let source = match fs::read_to_string(&input) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("read error: {e}");
            return ExitCode::from(1);
        }
    };
    let source_name = input.to_string_lossy();
    let result = match Context::new().eval_named(&source_name, &source) {
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
        match write_output(&destination, &document, incremental) {
            Ok(WriteOutcome::Written) => println!("wrote {}", destination.display()),
            Ok(WriteOutcome::Unchanged) => println!("unchanged {}", destination.display()),
            Err(e) => {
                eprintln!("write error: {e}");
                return ExitCode::from(1);
            }
        }
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::{
        WriteOutcome, render_inline_html, render_markdown, render_prose_html, split_source,
        write_output,
    };
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

        assert!(write_output(&destination, "new", false).is_err());
        assert_eq!(fs::read_to_string(&destination).expect("old output"), "old");
        assert_eq!(
            fs::read_to_string(&temporary).expect("reserved temporary output"),
            "reserved"
        );

        assert_eq!(
            write_output(&destination, "old", true).expect("unchanged output"),
            WriteOutcome::Unchanged
        );
        assert_eq!(
            fs::read_to_string(&temporary).expect("reserved temporary output"),
            "reserved"
        );

        fs::remove_file(&temporary).expect("release temporary output");
        assert_eq!(
            write_output(&destination, "new", true).expect("replace output"),
            WriteOutcome::Written
        );
        assert_eq!(fs::read_to_string(&destination).expect("new output"), "new");
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }

    #[test]
    fn literate_source_renders_markdown_prose_and_extracted_coffee() {
        let source = "# Rules\n\n    answer = 40\n    answer + 2\n";
        let (prose, code) = split_source(source);
        assert_eq!(prose.trim(), "# Rules");
        assert_eq!(code.trim(), "answer = 40\nanswer + 2");
        let rendered = render_markdown(source, "42");
        assert!(rendered.contains("````coffee\nanswer = 40"));
    }

    #[test]
    fn literate_html_renders_escaped_inline_code_spans() {
        assert_eq!(
            render_inline_html("Call `Context::eval` with `<source>`; keep unmatched ` safe."),
            "Call <code>Context::eval</code> with <code>&lt;source&gt;</code>; keep unmatched ` safe."
        );
        assert_eq!(
            render_inline_html("Write `` ` `` to describe a backtick."),
            "Write <code>`</code> to describe a backtick."
        );
        assert_eq!(
            render_prose_html("Use `qcoffee --check FILE`."),
            "<p>Use <code>qcoffee --check FILE</code>.</p>"
        );
    }
}

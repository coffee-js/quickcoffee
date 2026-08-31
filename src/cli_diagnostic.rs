//! Shared human-readable diagnostic rendering for QuickCoffee command-line tools.

use quickcoffee::{
    DiagnosticLabel, DiagnosticLabelKind, Error, ModuleLoader, ModuleSource, SourcePosition,
    SourceSpan, ValueKind,
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fmt::{self, Write},
};

const MAX_EXCERPT_SCALARS: usize = 120;
const CONTEXT_BEFORE_SCALARS: usize = 40;
const MAX_DETAIL_SCALARS: usize = 160;

/// Records the exact module sources returned by an already-authorized loader.
///
/// Diagnostics can therefore render the source that was compiled without treating an opaque
/// source name as a path or performing a second read that could observe different contents.
pub(crate) struct RecordingModuleLoader<'a> {
    inner: &'a dyn ModuleLoader,
    sources: RefCell<BTreeMap<String, String>>,
}

impl<'a> RecordingModuleLoader<'a> {
    pub(crate) fn new(inner: &'a dyn ModuleLoader) -> Self {
        Self {
            inner,
            sources: RefCell::new(BTreeMap::new()),
        }
    }

    pub(crate) fn record(&self, source: &ModuleSource) {
        self.sources
            .borrow_mut()
            .entry(source.name().to_owned())
            .or_insert_with(|| source.source().to_owned());
    }

    pub(crate) fn source(&self, name: &str) -> Option<String> {
        self.sources.borrow().get(name).cloned()
    }
}

impl ModuleLoader for RecordingModuleLoader<'_> {
    fn load(&self, specifier: &str, referrer: &str) -> Result<ModuleSource, Error> {
        let source = self.inner.load(specifier, referrer)?;
        self.record(&source);
        Ok(source)
    }
}

/// Renders an error, its ordered labels, compact source excerpts, and an optional hint.
///
/// `anonymous_source` supplies source text for labels without a source name. `load_named`
/// resolves opaque label names inside the authority already granted by the caller; this
/// renderer never interprets a source name as a path itself.
pub(crate) fn render_error(
    error: &Error,
    anonymous_source: Option<&str>,
    mut load_named: impl FnMut(&str) -> Option<String>,
) -> String {
    let mut rendered = error.to_string();
    let mut named_sources = BTreeMap::<String, Option<String>>::new();

    if let Some(details) = script_error_details(error) {
        rendered.push_str("\n  details: ");
        rendered.push_str(&details);
    }

    for label in error.labels() {
        rendered.push_str("\n  ");
        rendered.push_str(match label.kind {
            DiagnosticLabelKind::Primary => "-->",
            DiagnosticLabelKind::Secondary => ":::",
        });
        rendered.push(' ');
        rendered.push_str(&format_span(&label.span));

        let source = match label.span.source_name.as_deref() {
            Some(name) => named_sources
                .entry(name.to_owned())
                .or_insert_with(|| load_named(name))
                .as_deref(),
            None => anonymous_source,
        };
        if let Some(source) = source {
            render_excerpt(&mut rendered, label, source);
        }
        if let Some(message) = &label.message {
            rendered.push_str("\n      = ");
            rendered.push_str(message);
        }
    }

    if let Some(hint) = diagnostic_hint(error) {
        rendered.push_str("\n  help: ");
        rendered.push_str(hint);
    }
    rendered
}

fn script_error_details(error: &Error) -> Option<String> {
    let script_error = error.script_error()?;
    if matches!(script_error.code(), "runtime" | "throw") {
        return None;
    }
    let data = script_error.data();
    if data.kind() == ValueKind::Nil {
        return None;
    }
    let mut writer = DetailWriter::default();
    let _ = write!(&mut writer, "{data}");
    Some(writer.finish())
}

#[derive(Default)]
struct DetailWriter {
    rendered: String,
    scalars: usize,
    truncated: bool,
}

impl DetailWriter {
    fn push_scalar(&mut self, character: char) -> bool {
        if self.scalars == MAX_DETAIL_SCALARS {
            self.truncated = true;
            return false;
        }
        self.rendered.push(character);
        self.scalars += 1;
        true
    }

    fn push_escape(&mut self, escape: &str) -> bool {
        let length = escape.chars().count();
        if self.scalars.saturating_add(length) > MAX_DETAIL_SCALARS {
            self.truncated = true;
            return false;
        }
        self.rendered.push_str(escape);
        self.scalars += length;
        true
    }

    fn finish(mut self) -> String {
        if self.truncated {
            self.rendered.push('…');
        }
        self.rendered
    }
}

impl fmt::Write for DetailWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.truncated {
            return Ok(());
        }
        for character in value.chars() {
            let written = match character {
                '\n' => self.push_escape("\\n"),
                '\r' => self.push_escape("\\r"),
                '\t' => self.push_escape("\\t"),
                character if character.is_control() => self.push_scalar('�'),
                character => self.push_scalar(character),
            };
            if !written {
                break;
            }
        }
        Ok(())
    }
}

fn format_span(span: &SourceSpan) -> String {
    let mut location = span.source_name.as_deref().unwrap_or("<input>").to_owned();
    push_position(&mut location, span.start);
    if let Some(end) = span.end {
        location.push('-');
        location.push_str(&end.line.to_string());
        if let Some(column) = end.column {
            location.push(':');
            location.push_str(&column.to_string());
        }
    }
    location
}

fn push_position(target: &mut String, position: SourcePosition) {
    target.push(':');
    target.push_str(&position.line.to_string());
    if let Some(column) = position.column {
        target.push(':');
        target.push_str(&column.to_string());
    }
}

fn render_excerpt(rendered: &mut String, label: &DiagnosticLabel, source: &str) {
    let Some(line) = source
        .split('\n')
        .nth(label.span.start.line.saturating_sub(1))
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
    else {
        return;
    };
    let scalars = line.chars().map(display_scalar).collect::<Vec<_>>();
    let target = label
        .span
        .start
        .column
        .map_or(0, |column| column.saturating_sub(1))
        .min(scalars.len());
    let window_start = if scalars.len() <= MAX_EXCERPT_SCALARS {
        0
    } else {
        target
            .saturating_sub(CONTEXT_BEFORE_SCALARS)
            .min(scalars.len() - MAX_EXCERPT_SCALARS)
    };
    let window_end = scalars
        .len()
        .min(window_start.saturating_add(MAX_EXCERPT_SCALARS));
    let leading_ellipsis = usize::from(window_start > 0);
    let mut excerpt = String::new();
    if leading_ellipsis == 1 {
        excerpt.push('…');
    }
    excerpt.extend(scalars[window_start..window_end].iter());
    if window_end < scalars.len() {
        excerpt.push('…');
    }

    let line_number = label.span.start.line;
    let width = line_number.to_string().len();
    rendered.push_str(&format!(
        "\n {line_number:>width$} | {excerpt}",
        width = width
    ));

    let Some(start_column) = label.span.start.column else {
        return;
    };
    let marker_start = start_column
        .saturating_sub(1)
        .saturating_sub(window_start)
        .saturating_add(leading_ellipsis)
        .min(excerpt.chars().count());
    let requested_width = label
        .span
        .end
        .filter(|end| end.line == label.span.start.line)
        .and_then(|end| end.column)
        .map_or(1, |end_column| {
            end_column.saturating_sub(start_column).max(1)
        });
    let available_width = excerpt.chars().count().saturating_sub(marker_start).max(1);
    let marker_width = requested_width.min(available_width);
    let marker = match label.kind {
        DiagnosticLabelKind::Primary => '^',
        DiagnosticLabelKind::Secondary => '-',
    };
    rendered.push_str(&format!(
        "\n {:>width$} | {}{}",
        "",
        " ".repeat(marker_start),
        marker.to_string().repeat(marker_width),
        width = width
    ));
}

fn display_scalar(character: char) -> char {
    match character {
        '\t' => ' ',
        character if character.is_control() => '�',
        character => character,
    }
}

fn diagnostic_hint(error: &Error) -> Option<&'static str> {
    if error.kind() != quickcoffee::ErrorKind::Runtime {
        return None;
    }
    let message = error.message();
    if message == "cannot mix number, integer, and decimal operands" {
        Some("convert both operands explicitly with number(...), integer(...), or decimal(...)")
    } else if message.starts_with("map key '") && message.ends_with("' not found") {
        Some("test membership with 'key of map' before reading a possibly missing key")
    } else if message.starts_with("expected ") && message.contains(" arguments, got ") {
        Some("check the function parameter count; QuickCoffee does not add or discard arguments")
    } else if message.contains(" expects ") {
        Some(
            "check the required argument or operand shape and strict value types; no coercion is applied",
        )
    } else {
        None
    }
}

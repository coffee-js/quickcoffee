//! Deterministic CSON and JSON conversion tool.

use quickcoffee::{
    CsonError, CsonErrorCode, CsonLimits, JsonError, JsonErrorCode, ResourceLimit, ResourceLimits,
    encode_json_with_limits, parse_cson_with_limits, parse_json_with_limits, to_cson_with_limits,
};
use std::{
    env, fs,
    io::{self, Read, Write},
    process::ExitCode,
};

const DIAGNOSTIC_SCHEMA: &str = "quickcoffee.qcson-diagnostic.v1";

#[derive(Clone, Copy)]
enum Direction {
    ToJson,
    ToCson,
}

impl Direction {
    fn source_format(self) -> &'static str {
        match self {
            Self::ToJson => "cson",
            Self::ToCson => "json",
        }
    }

    fn target_format(self) -> &'static str {
        match self {
            Self::ToJson => "json",
            Self::ToCson => "cson",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiagnosticFormat {
    Human,
    Json,
}

struct Options {
    direction: Direction,
    input: String,
    diagnostic_format: DiagnosticFormat,
    max_input_bytes: usize,
    max_output_bytes: usize,
}

struct Diagnostic {
    stage: &'static str,
    format: Option<&'static str>,
    code: &'static str,
    message: String,
    source: Option<String>,
    byte_start: Option<usize>,
    byte_end: Option<usize>,
    line: Option<usize>,
    column: Option<usize>,
    limit: Option<&'static str>,
}

struct UsageFailure {
    diagnostic: Box<Diagnostic>,
    format: DiagnosticFormat,
}

fn usage() -> &'static str {
    "Usage: qcson [--diagnostic-format human|json] [--max-input-bytes N] [--max-output-bytes N] <to-json|to-cson> <FILE|->\n       qcson --version\n\nConverts data only; it never executes CSON or QuickCoffee.\n仅转换数据；绝不执行 CSON 或 QuickCoffee。"
}

fn parse_options() -> Result<Option<Options>, UsageFailure> {
    let mut args = env::args().skip(1);
    let mut direction = None;
    let mut input = None;
    let mut diagnostic_format = DiagnosticFormat::Human;
    let mut max_input_bytes = None;
    let mut max_output_bytes = None;

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--help" | "-h" => {
                eprintln!("{}", usage());
                return Ok(None);
            }
            "--version" => {
                println!("qcson {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "--diagnostic-format" => match args.next().as_deref() {
                Some("human") => diagnostic_format = DiagnosticFormat::Human,
                Some("json") => diagnostic_format = DiagnosticFormat::Json,
                _ => {
                    return Err(usage_error(
                        diagnostic_format,
                        "--diagnostic-format requires human or json",
                    ));
                }
            },
            "--max-input-bytes" => {
                max_input_bytes = Some(parse_limit(
                    args.next(),
                    "--max-input-bytes",
                    diagnostic_format,
                )?);
            }
            "--max-output-bytes" => {
                max_output_bytes = Some(parse_limit(
                    args.next(),
                    "--max-output-bytes",
                    diagnostic_format,
                )?);
            }
            "to-json" if direction.is_none() => direction = Some(Direction::ToJson),
            "to-cson" if direction.is_none() => direction = Some(Direction::ToCson),
            value if input.is_none() && direction.is_some() => input = Some(value.to_owned()),
            _ => return Err(usage_error(diagnostic_format, "unexpected argument")),
        }
    }

    let Some(direction) = direction else {
        return Err(usage_error(
            diagnostic_format,
            "a to-json or to-cson direction is required",
        ));
    };
    let Some(input) = input else {
        return Err(usage_error(
            diagnostic_format,
            "an input file or - is required",
        ));
    };
    let default_input = match direction {
        Direction::ToJson => CsonLimits::default().max_input_bytes(),
        Direction::ToCson => ResourceLimits::default().max_json_input_bytes(),
    };
    let default_output = match direction {
        Direction::ToJson => ResourceLimits::default().max_json_output_bytes(),
        Direction::ToCson => CsonLimits::default().max_output_bytes(),
    };
    Ok(Some(Options {
        direction,
        input,
        diagnostic_format,
        max_input_bytes: max_input_bytes.unwrap_or(default_input),
        max_output_bytes: max_output_bytes.unwrap_or(default_output),
    }))
}

fn parse_limit(
    value: Option<String>,
    option: &str,
    diagnostic_format: DiagnosticFormat,
) -> Result<usize, UsageFailure> {
    value
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| {
            usage_error(
                diagnostic_format,
                format!("{option} requires a non-negative integer"),
            )
        })
}

fn usage_error(diagnostic_format: DiagnosticFormat, message: impl Into<String>) -> UsageFailure {
    let message = message.into();
    UsageFailure {
        diagnostic: Box::new(Diagnostic {
            stage: "usage",
            format: None,
            code: "E_QCSON_USAGE",
            message: if diagnostic_format == DiagnosticFormat::Human {
                format!("{message}\n{}", usage())
            } else {
                message
            },
            source: None,
            byte_start: None,
            byte_end: None,
            line: None,
            column: None,
            limit: None,
        }),
        format: diagnostic_format,
    }
}

fn read_source(path: &str, limit: usize) -> Result<String, ReadFailure> {
    if path == "-" {
        return read_limited(io::stdin().lock(), limit);
    }
    let file = fs::File::open(path).map_err(|error| ReadFailure::Io(error.to_string()))?;
    if file
        .metadata()
        .map_err(|error| ReadFailure::Io(error.to_string()))?
        .len()
        > limit as u64
    {
        return Err(ReadFailure::InputLimit);
    }
    read_limited(file, limit)
}

enum ReadFailure {
    Io(String),
    InputLimit,
}

fn read_limited(reader: impl Read, limit: usize) -> Result<String, ReadFailure> {
    let mut source = String::new();
    reader
        .take((limit as u64).saturating_add(1))
        .read_to_string(&mut source)
        .map_err(|error| ReadFailure::Io(error.to_string()))?;
    if source.len() > limit {
        Err(ReadFailure::InputLimit)
    } else {
        Ok(source)
    }
}

fn convert(options: &Options, source: &str) -> Result<String, Box<Diagnostic>> {
    match options.direction {
        Direction::ToJson => {
            let value = parse_cson_with_limits(
                source,
                CsonLimits::default().with_max_input_bytes(options.max_input_bytes),
            )
            .map_err(|error| Box::new(cson_diagnostic("parse", "cson", &options.input, error)))?;
            let encoder_limit = options.max_output_bytes.saturating_sub(1);
            let encoded = encode_json_with_limits(
                &value,
                ResourceLimits::default().with_max_json_output_bytes(encoder_limit),
            )
            .map_err(|error| {
                if error.resource_limit() == Some(ResourceLimit::JsonOutputBytes) {
                    Box::new(json_output_limit(&options.input, options.max_output_bytes))
                } else {
                    Box::new(json_diagnostic(
                        "encode",
                        "json",
                        &options.input,
                        source,
                        error,
                    ))
                }
            })?;
            if encoded.len().saturating_add(1) > options.max_output_bytes {
                return Err(Box::new(json_output_limit(
                    &options.input,
                    options.max_output_bytes,
                )));
            }
            Ok(format!("{encoded}\n"))
        }
        Direction::ToCson => {
            let value = parse_json_with_limits(
                source,
                ResourceLimits::default().with_max_json_input_bytes(options.max_input_bytes),
            )
            .map_err(|error| {
                Box::new(json_diagnostic(
                    "parse",
                    "json",
                    &options.input,
                    source,
                    error,
                ))
            })?;
            to_cson_with_limits(
                &value,
                CsonLimits::default().with_max_output_bytes(options.max_output_bytes),
            )
            .map_err(|error| Box::new(cson_diagnostic("encode", "cson", &options.input, error)))
        }
    }
}

fn read_diagnostic(options: &Options, failure: ReadFailure) -> Diagnostic {
    match failure {
        ReadFailure::Io(message) => Diagnostic {
            stage: "read",
            format: Some(options.direction.source_format()),
            code: "E_QCSON_READ",
            message: format!("cannot read input: {message}"),
            source: Some(options.input.clone()),
            byte_start: None,
            byte_end: None,
            line: None,
            column: None,
            limit: None,
        },
        ReadFailure::InputLimit => match options.direction {
            Direction::ToJson => Diagnostic {
                stage: "read",
                format: Some("cson"),
                code: CsonErrorCode::InputLimit.as_str(),
                message: format!("CSON input exceeds {} bytes", options.max_input_bytes),
                source: Some(options.input.clone()),
                byte_start: None,
                byte_end: None,
                line: None,
                column: None,
                limit: Some("cson_input_bytes"),
            },
            Direction::ToCson => Diagnostic {
                stage: "read",
                format: Some("json"),
                code: JsonErrorCode::Resource.as_str(),
                message: format!("JSON input exceeds {} bytes", options.max_input_bytes),
                source: Some(options.input.clone()),
                byte_start: None,
                byte_end: None,
                line: None,
                column: None,
                limit: Some("json_input_bytes"),
            },
        },
    }
}

fn cson_diagnostic(
    stage: &'static str,
    format: &'static str,
    source: &str,
    error: CsonError,
) -> Diagnostic {
    let has_input_position = stage == "parse";
    Diagnostic {
        stage,
        format: Some(format),
        code: error.code().as_str(),
        message: error.message().to_owned(),
        source: Some(source.to_owned()),
        byte_start: has_input_position.then(|| error.byte_range().start),
        byte_end: has_input_position.then(|| error.byte_range().end),
        line: has_input_position.then_some(error.span().start.line),
        column: has_input_position
            .then_some(error.span().start.column)
            .flatten(),
        limit: cson_limit(error.code()),
    }
}

fn cson_limit(code: CsonErrorCode) -> Option<&'static str> {
    match code {
        CsonErrorCode::InputLimit => Some("cson_input_bytes"),
        CsonErrorCode::OutputLimit => Some("cson_output_bytes"),
        CsonErrorCode::StringLimit => Some("cson_string_bytes"),
        CsonErrorCode::ValueLimit => Some("cson_values"),
        CsonErrorCode::ContainerLimit => Some("cson_container_items"),
        CsonErrorCode::DepthLimit => Some("cson_nesting_depth"),
        CsonErrorCode::WorkLimit => Some("cson_work_units"),
        CsonErrorCode::DiagnosticLimit => Some("cson_diagnostics"),
        _ => None,
    }
}

fn json_diagnostic(
    stage: &'static str,
    format: &'static str,
    source_name: &str,
    source: &str,
    error: JsonError,
) -> Diagnostic {
    let byte_offset = error.byte_offset();
    let (line, column) = byte_offset
        .map(|offset| source_position(source, offset))
        .map_or((None, None), |(line, column)| (Some(line), Some(column)));
    Diagnostic {
        stage,
        format: Some(format),
        code: error.code().as_str(),
        message: error.message().to_owned(),
        source: Some(source_name.to_owned()),
        byte_start: byte_offset,
        byte_end: byte_offset,
        line,
        column,
        limit: error.resource_limit().map(resource_limit_name),
    }
}

fn json_output_limit(source: &str, limit: usize) -> Diagnostic {
    Diagnostic {
        stage: "encode",
        format: Some("json"),
        code: JsonErrorCode::Resource.as_str(),
        message: format!("JSON output including final LF exceeds {limit} bytes"),
        source: Some(source.to_owned()),
        byte_start: None,
        byte_end: None,
        line: None,
        column: None,
        limit: Some("json_output_bytes"),
    }
}

fn resource_limit_name(limit: ResourceLimit) -> &'static str {
    match limit {
        ResourceLimit::JsonInputBytes => "json_input_bytes",
        ResourceLimit::JsonOutputBytes => "json_output_bytes",
        ResourceLimit::JsonStringBytes => "json_string_bytes",
        ResourceLimit::JsonContainerItems => "json_container_items",
        ResourceLimit::JsonValueCount => "json_values",
        ResourceLimit::JsonNestingDepth => "json_nesting_depth",
        ResourceLimit::IntegerBits => "integer_bits",
        ResourceLimit::DecimalCoefficientBits => "decimal_coefficient_bits",
        ResourceLimit::DecimalScale => "decimal_scale",
        _ => "other",
    }
}

fn source_position(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    let mut cursor = 0;
    while cursor < offset.min(source.len()) {
        let character = source[cursor..]
            .chars()
            .next()
            .expect("cursor remains on a UTF-8 boundary");
        cursor += character.len_utf8();
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn render_diagnostic(diagnostic: &Diagnostic, format: DiagnosticFormat) -> String {
    if format == DiagnosticFormat::Json {
        return diagnostic_json(diagnostic);
    }
    let location = match (
        diagnostic.source.as_deref(),
        diagnostic.line,
        diagnostic.column,
    ) {
        (Some(source), Some(line), Some(column)) => format!("{source}:{line}:{column}: "),
        (Some(source), _, _) => format!("{source}: "),
        _ => String::new(),
    };
    format!("{}{}: {}", location, diagnostic.code, diagnostic.message)
}

fn diagnostic_json(diagnostic: &Diagnostic) -> String {
    format!(
        "{{\"schema\":\"{DIAGNOSTIC_SCHEMA}\",\"ok\":false,\"stage\":\"{}\",\"format\":{},\"code\":\"{}\",\"message\":\"{}\",\"source\":{},\"byte_start\":{},\"byte_end\":{},\"line\":{},\"column\":{},\"limit\":{}}}",
        diagnostic.stage,
        json_optional_string(diagnostic.format),
        diagnostic.code,
        json_escape(&diagnostic.message),
        json_optional_string(diagnostic.source.as_deref()),
        json_optional_usize(diagnostic.byte_start),
        json_optional_usize(diagnostic.byte_end),
        json_optional_usize(diagnostic.line),
        json_optional_usize(diagnostic.column),
        json_optional_string(diagnostic.limit),
    )
}

fn json_optional_string(value: Option<&str>) -> String {
    value.map_or_else(
        || "null".to_owned(),
        |value| format!("\"{}\"", json_escape(value)),
    )
}

fn json_optional_usize(value: Option<usize>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| value.to_string())
}

fn json_escape(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                output.push_str(&format!("\\u{:04x}", u32::from(character)));
            }
            character => output.push(character),
        }
    }
    output
}

fn write_failure(format: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        stage: "write",
        format: Some(format),
        code: "E_QCSON_WRITE",
        message: message.into(),
        source: None,
        byte_start: None,
        byte_end: None,
        line: None,
        column: None,
        limit: None,
    }
}

fn main() -> ExitCode {
    let options = match parse_options() {
        Ok(Some(options)) => options,
        Ok(None) => return ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", render_diagnostic(&error.diagnostic, error.format));
            return ExitCode::from(2);
        }
    };

    let source = match read_source(&options.input, options.max_input_bytes) {
        Ok(source) => source,
        Err(error) => {
            eprintln!(
                "{}",
                render_diagnostic(&read_diagnostic(&options, error), options.diagnostic_format)
            );
            return ExitCode::from(1);
        }
    };
    let output = match convert(&options, &source) {
        Ok(output) => output,
        Err(diagnostic) => {
            eprintln!(
                "{}",
                render_diagnostic(&diagnostic, options.diagnostic_format)
            );
            return ExitCode::from(1);
        }
    };
    if let Err(error) = io::stdout().lock().write_all(output.as_bytes()) {
        eprintln!(
            "{}",
            render_diagnostic(
                &write_failure(
                    options.direction.target_format(),
                    format!("cannot write output: {error}"),
                ),
                options.diagnostic_format,
            )
        );
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

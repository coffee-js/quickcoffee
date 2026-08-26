use crate::Error;
use std::borrow::Cow;

pub(crate) struct PreparedSource<'a> {
    pub(crate) text: Cow<'a, str>,
    pub(crate) columns_are_precise: bool,
}

pub(crate) fn prepare<'a>(
    source_name: Option<&str>,
    source: &'a str,
) -> Result<PreparedSource<'a>, Error> {
    if source_name.is_some_and(|name| name.ends_with(".litcoffee")) {
        Ok(PreparedSource {
            text: Cow::Owned(preprocess_literate(source)?),
            columns_are_precise: false,
        })
    } else {
        Ok(PreparedSource {
            text: Cow::Borrowed(source),
            columns_are_precise: true,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LiterateMargin {
    Spaces,
    Tab,
}

fn code_line(line: &str) -> Option<(LiterateMargin, &str)> {
    line.strip_prefix("    ")
        .map(|line| (LiterateMargin::Spaces, line))
        .or_else(|| {
            line.strip_prefix('\t')
                .map(|line| (LiterateMargin::Tab, line))
        })
}

fn preprocess_literate(source: &str) -> Result<String, Error> {
    let mut output = String::with_capacity(source.len());
    let mut in_code = false;
    let mut code_may_start = true;
    let mut margin = None;

    for (index, physical_line) in source.split_inclusive('\n').enumerate() {
        let has_newline = physical_line.ends_with('\n');
        let line = physical_line.strip_suffix('\n').unwrap_or(physical_line);
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.trim().is_empty() {
            output.push('\n');
            code_may_start = true;
            continue;
        }

        if let Some((line_margin, code)) = code_line(line) {
            if in_code || code_may_start {
                if margin.is_some_and(|margin| margin != line_margin) {
                    return Err(
                        Error::parse("inconsistent literate code indentation").at_line(index + 1)
                    );
                }
                margin.get_or_insert(line_margin);
                output.push_str(code);
                if has_newline {
                    output.push('\n');
                }
                in_code = true;
                code_may_start = false;
                continue;
            }
        }

        if has_newline {
            output.push('\n');
        }
        in_code = false;
        code_may_start = false;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{prepare, preprocess_literate};

    #[test]
    fn literate_source_keeps_lines_and_strips_one_markdown_code_margin() {
        let source = "# Rules\n\n    answer = 40\n    if true\n      answer += 2\n    answer\n";
        assert_eq!(
            preprocess_literate(source).unwrap(),
            "\n\nanswer = 40\nif true\n  answer += 2\nanswer\n"
        );
        let prepared = prepare(Some("rules.litcoffee"), source).unwrap();
        assert!(!prepared.columns_are_precise);
        assert_eq!(
            prepared.text,
            "\n\nanswer = 40\nif true\n  answer += 2\nanswer\n"
        );
        assert_eq!(prepared.text.lines().count(), source.lines().count());
        crate::parser::parse_with_columns(&prepared.text, prepared.columns_are_precise).unwrap();
    }

    #[test]
    fn literate_source_requires_separation_and_consistent_margin_style() {
        assert_eq!(
            preprocess_literate("Paragraph\n    not_code = true\n").unwrap(),
            "\n\n"
        );
        let error = preprocess_literate("    one = 1\n\n\ttwo = 2\n").unwrap_err();
        assert_eq!(error.position().map(|position| position.line), Some(3));
    }
}

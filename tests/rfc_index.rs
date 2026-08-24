use std::{fs, path::Path};

#[test]
fn rfc_numbers_and_index_references_are_consistent() {
    let mut entries = fs::read_dir("RFCs")
        .expect("RFC directory exists")
        .map(|entry| entry.expect("RFC entry is readable").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("md"))
        .collect::<Vec<_>>();
    entries.sort();
    assert!(!entries.is_empty());

    let mut numbered = Vec::with_capacity(entries.len());
    for path in entries {
        let name = path.file_name().and_then(|name| name.to_str()).unwrap();
        let number = name
            .split_once('-')
            .and_then(|(number, _)| number.parse::<u32>().ok())
            .unwrap_or_else(|| panic!("RFC filename must start with a number: {name}"));
        let source = fs::read_to_string(&path).expect("RFC source is readable");
        assert!(source.starts_with("# RFC "), "RFC {name} needs a title");
        assert!(
            source.contains("状态：") || source.contains("Status:"),
            "RFC {name} needs a status field"
        );
        assert!(
            !source.contains("状态：实现中") && !source.contains("Status: Implementing"),
            "RFC {name} is still marked as implementing; completed scope RFCs must be adopted"
        );
        numbered.push((number, name.to_owned()));
    }

    let latest = numbered.last().unwrap().0;
    for (expected, (actual, _)) in (0..=latest).zip(numbered.iter()) {
        assert_eq!(
            *actual, expected,
            "RFC numbering has a gap before {expected:04}"
        );
    }
    let latest_name = &numbered.last().unwrap().1;
    let readme = fs::read_to_string(Path::new("README.md")).expect("README exists");
    assert!(
        readme.contains(latest_name),
        "README must link the latest RFC"
    );
    let scope = fs::read_to_string("RFCs/0000-project-scope.md").expect("scope RFC exists");
    assert!(
        scope.contains(&format!("RFC {latest:04}")),
        "scope RFC must mention the latest RFC"
    );
}

#[test]
fn package_manifest_declares_the_documented_msrv() {
    let manifest = fs::read_to_string("Cargo.toml").expect("Cargo manifest exists");
    let rust_version = manifest.lines().find_map(|line| {
        let line = line.split('#').next()?.trim();
        let (key, value) = line.split_once('=')?;
        (key.trim() == "rust-version").then(|| value.trim().trim_matches('"'))
    });
    assert!(
        rust_version == Some("1.85"),
        "Cargo.toml must declare the RFC 0110 MSRV"
    );
}

#[test]
fn frontend_stage_ownership_is_explicit() {
    let ast = fs::read_to_string("src/ast.rs").expect("AST module exists");
    let parser = fs::read_to_string("src/parser.rs").expect("parser module exists");
    let lowering = fs::read_to_string("src/lowering.rs").expect("lowering module exists");
    let bytecode = fs::read_to_string("src/bytecode.rs").expect("bytecode module exists");

    assert!(ast.contains("pub(crate) enum Expr"));
    assert!(parser.contains("ast::{Binary, Expr"));
    assert!(!parser.contains("enum Expr"));

    assert!(lowering.contains("struct Compiler"));
    assert!(lowering.contains("pub(crate) fn compile_mapped"));
    assert!(!bytecode.contains("struct Compiler"));
    assert!(!bytecode.contains("pub(crate) fn compile_mapped"));
    assert!(bytecode.contains("impl Chunk"));
}

#[test]
fn coffeescript_feature_matrix_covers_the_official_language_reference() {
    let matrix = fs::read_to_string("docs/coffeescript-2016-matrix.md")
        .expect("CoffeeScript feature matrix exists");
    let sections = [
        "Functions",
        "Objects and Arrays",
        "Lexical Scoping and Variable Safety",
        "If, Else, Unless, and Conditional Assignment",
        "Splats…",
        "Loops and Comprehensions",
        "Array Slicing and Splicing with Ranges",
        "Everything is an Expression (at least, as much as possible)",
        "Operators and Aliases",
        "The Existential Operator",
        "Classes, Inheritance, and Super",
        "Destructuring Assignment",
        "Bound Functions, Generator Functions",
        "Embedded JavaScript",
        "Switch/When/Else",
        "Try/Catch/Finally",
        "Chained Comparisons",
        "String Interpolation, Block Strings, and Block Comments",
        "Tagged Template Literals",
        "Block Regular Expressions",
        "Modules",
    ];

    for section in sections {
        let marker = format!("| {section} |");
        assert_eq!(
            matrix.matches(&marker).count(),
            1,
            "matrix must contain exactly one row for official section {section:?}"
        );
    }

    let rows = matrix
        .lines()
        .filter(|line| line.starts_with("| ") && !line.starts_with("|---"))
        .skip(1)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), sections.len(), "matrix has an unexpected row");
    for row in rows {
        let columns = row
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        assert_eq!(columns.len(), 4, "matrix row must have four columns: {row}");
        assert!(
            matches!(columns[1], "Implement" | "Adapt" | "Reject"),
            "matrix row has an invalid status: {row}"
        );
        assert!(
            columns[3].contains("(../RFCs/"),
            "matrix row must link normative RFC evidence: {row}"
        );
    }
}

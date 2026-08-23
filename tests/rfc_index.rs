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

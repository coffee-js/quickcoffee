use quickcoffee::{
    Context, Engine, ErrorKind, MemoryModuleLoader, ModuleLoader, ResourceLimit,
    RestrictedFileModuleLoader, Value,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

fn module_temp(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "quickcoffee-module-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn named_static_imports_and_exports_keep_module_globals_private() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { answer, double } from 'math'\nhidden = 1\nexport result = double(answer)",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert(
        "math",
        "answer = 21\ntwice = (value) -> value * 2\nexport { answer, twice as double }",
    );

    let mut context = Context::new();
    let exports = context.run_module(&main, &loader).unwrap();
    assert_eq!(exports.get("result").and_then(Value::as_number), Some(42.));
    assert_eq!(exports.len(), 1);
    assert!(context.get_global("hidden").is_none());
    assert!(context.get_global("answer").is_none());
    let stats = context.last_execution();
    assert!(stats.name_loads > 0);
    assert!(stats.calls > 0);
    assert!(stats.value_allocations > 0);
    assert!(stats.environment_allocations > 0);
}

#[test]
fn module_loading_caches_dependencies_and_reports_missing_names() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { value } from 'shared'\nimport { value as again } from 'shared'\nexport sum = value + again",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert("shared", "export value = 21");
    let exports = Context::new().run_module(&main, &loader).unwrap();
    assert_eq!(exports.get("sum").and_then(Value::as_number), Some(42.));

    let missing = engine
        .compile_module(
            "missing",
            "import { absent } from 'shared'\nexport value = absent",
        )
        .unwrap();
    let error = Context::new().run_module(&missing, &loader).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert!(error.message().contains("does not export absent"));
}

#[test]
fn runtime_errors_in_dependency_functions_keep_the_dependency_name() {
    let main = Engine::new()
        .compile_module(
            "main",
            "import { fail } from 'dependency'\nexport result = fail(1)",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert(
        "dependency",
        "fail = (value) -> value + 'x'\nexport { fail }",
    );
    let error = Context::new().run_module(&main, &loader).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("dependency")
    );
    assert_eq!(error.labels()[0].span.start.line, 1);
    assert_eq!(error.labels().len(), 2);
    assert_eq!(
        error.labels()[1].kind,
        quickcoffee::DiagnosticLabelKind::Secondary
    );
    assert_eq!(
        error.labels()[1].message.as_deref(),
        Some("called from here")
    );
    assert_eq!(error.labels()[1].span.source_name.as_deref(), Some("main"));
    assert_eq!(error.labels()[1].span.start.line, 2);
}

#[test]
fn module_cycles_are_rejected_and_resources_cover_dependencies() {
    let engine = Engine::new();
    let main = engine
        .compile_module("main", "import { value } from 'a'\nexport value = value")
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert("a", "import { value } from 'b'\nexport value = value");
    loader.insert("b", "import { value } from 'a'\nexport value = value");
    let error = Context::new().run_module(&main, &loader).unwrap_err();
    assert!(
        error
            .message()
            .contains("circular module dependency: main -> a -> b -> a")
    );

    loader.insert("a", "loop 1");
    let error = Context::new()
        .with_fuel(8)
        .run_module(&main, &loader)
        .unwrap_err();
    assert_eq!(error.resource_limit(), Some(ResourceLimit::Fuel));
}

#[test]
fn module_directives_are_not_accepted_by_single_file_compilation() {
    let error = Engine::new()
        .compile_program_named("virtual://single.qc", "value = 1\nexport answer = 42")
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Verify);
    assert_eq!(
        error.to_string(),
        "verify error: module directives require Engine::compile_module"
    );
    let span = &error.labels()[0].span;
    assert_eq!(span.source_name.as_deref(), Some("virtual://single.qc"));
    assert_eq!(span.start.line, 2);
    assert_eq!(span.start.column, Some(1));
    assert_eq!(
        span.end,
        Some(quickcoffee::SourcePosition {
            line: 2,
            column: Some(7),
        })
    );
}

#[test]
fn module_parse_errors_carry_the_host_canonical_name() {
    let error = Engine::new()
        .compile_module("pkg://rules/../invoice", "value = 1\n@")
        .unwrap_err();
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("pkg://rules/../invoice")
    );
}

#[test]
fn duplicate_module_exports_point_to_the_later_directive() {
    let error = Engine::new()
        .compile_module(
            "pkg://rules/invoice",
            "export answer = 41\nexport answer = 42",
        )
        .unwrap_err();
    assert_eq!(error.message(), "duplicate module export: answer");
    let span = &error.labels()[0].span;
    assert_eq!(span.source_name.as_deref(), Some("pkg://rules/invoice"));
    assert_eq!(span.start.line, 2);
    assert_eq!(span.start.column, Some(1));
    assert_eq!(
        span.end,
        Some(quickcoffee::SourcePosition {
            line: 2,
            column: Some(7),
        })
    );
}

#[test]
fn memory_loader_only_resolves_exact_names() {
    let loader = MemoryModuleLoader::new();
    let error = loader.load("./missing", "main").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert_eq!(error.message(), "module not found: ./missing");
}

#[test]
fn restricted_file_loader_resolves_nested_relative_modules() {
    let root = module_temp("relative");
    fs::create_dir_all(root.join("app/lib")).unwrap();
    fs::create_dir_all(root.join("shared")).unwrap();
    fs::write(
        root.join("app/main.qc"),
        "import { double } from './lib/math'\nimport { base } from '../shared/value.qc'\nexport result = double(base)",
    )
    .unwrap();
    fs::write(
        root.join("app/lib/math.qc"),
        "export double = (value) -> value * 2",
    )
    .unwrap();
    fs::write(root.join("shared/value.qc"), "export base = 21").unwrap();

    let loader = RestrictedFileModuleLoader::new(&root).unwrap();
    let source = loader.load_entry("app/main").unwrap();
    assert_eq!(source.name(), "app/main.qc");
    let main = Engine::new()
        .compile_module(source.name(), source.source())
        .unwrap();
    let exports = Context::new().run_module(&main, &loader).unwrap();
    assert_eq!(exports.get("result").and_then(Value::as_number), Some(42.));

    let normalized = loader.load("./lib/../lib/math", "app/main.qc").unwrap();
    assert_eq!(normalized.name(), "app/lib/math.qc");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn restricted_file_loader_rejects_ambiguous_and_escaping_paths() {
    let root = module_temp("invalid");
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(root.join("app/main.qc"), "export value = 1").unwrap();
    let loader = RestrictedFileModuleLoader::new(&root).unwrap();

    for specifier in ["package", "/absolute", "./windows\\module", "./wrong.txt"] {
        let error = loader.load(specifier, "app/main.qc").unwrap_err();
        assert_eq!(
            error.message(),
            format!("invalid module specifier: {specifier}")
        );
    }
    let error = loader.load("../../outside", "app/main.qc").unwrap_err();
    assert_eq!(
        error.message(),
        "module path escapes configured root: ../../outside"
    );
    let error = loader.load("./missing", "app/main.qc").unwrap_err();
    assert_eq!(error.message(), "module not found: app/missing.qc");
    let error = loader.load("./main", "../app/main.qc").unwrap_err();
    assert_eq!(error.message(), "invalid module referrer: ../app/main.qc");
    let error = loader.load_entry("../outside").unwrap_err();
    assert_eq!(error.message(), "invalid module entry: ../outside");
    let error = loader.load_entry("app/main.txt").unwrap_err();
    assert_eq!(error.message(), "invalid module entry: app/main.txt");
    fs::write(root.join("app/binary.qc"), [0xff]).unwrap();
    let error = loader.load_entry("app/binary").unwrap_err();
    assert_eq!(
        error.message(),
        "module source is not readable UTF-8: app/binary.qc"
    );
    fs::create_dir(root.join("app/directory.qc")).unwrap();
    let error = loader.load_entry("app/directory").unwrap_err();
    assert_eq!(error.message(), "invalid module target: app/directory.qc");
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn restricted_file_loader_canonicalizes_aliases_and_rejects_symlink_escape() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let base = module_temp("symlink");
    let root = base.join("root");
    fs::create_dir_all(root.join("real")).unwrap();
    fs::write(root.join("real/module.qc"), "export value = 42").unwrap();
    fs::write(base.join("outside.qc"), "export value = 0").unwrap();
    fs::write(root.join("real:module.qc"), "export value = 1").unwrap();
    fs::write(root.join("real\\module.qc"), "export value = 2").unwrap();
    symlink("real/module.qc", root.join("alias.qc")).unwrap();
    symlink(base.join("outside.qc"), root.join("escape.qc")).unwrap();
    symlink("real:module.qc", root.join("colon-alias.qc")).unwrap();
    symlink("real\\module.qc", root.join("backslash-alias.qc")).unwrap();

    let loader = RestrictedFileModuleLoader::new(&root).unwrap();
    let alias = loader.load_entry("alias").unwrap();
    assert_eq!(alias.name(), "real/module.qc");
    let error = loader.load_entry("escape").unwrap_err();
    assert_eq!(
        error.message(),
        "module path escapes configured root: escape.qc"
    );
    for alias in ["colon-alias", "backslash-alias"] {
        let error = loader.load_entry(alias).unwrap_err();
        assert_eq!(
            error.message(),
            format!("invalid module target: {alias}.qc")
        );
    }
    let locked = root.join("locked.qc");
    fs::write(&locked, "export value = 3").unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let error = loader.load_entry("locked").unwrap_err();
    assert_eq!(error.message(), "module source is not readable: locked.qc");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o600)).unwrap();
    let _ = fs::remove_dir_all(base);
}

use quickcoffee::{
    Context, Engine, ErrorKind, MemoryModuleLoader, ModuleLoader, ResourceLimit, ResourceLimits,
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
    assert!(stats.managed_objects_allocated > 0);
    assert!(stats.managed_bytes_allocated > 0);
}

#[test]
fn modules_export_classes_without_exposing_their_receiver_state() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { Point } from 'geometry'\npoint = new Point(42)\nexport result = point.value()",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert(
        "geometry",
        "class Point\n  constructor: (@x) ->\n  value: -> @x\nexport { Point }",
    );

    let exports = Context::new().run_module(&main, &loader).unwrap();
    assert_eq!(exports.get("result").and_then(Value::as_number), Some(42.));
}

#[test]
fn modules_can_privately_extend_imported_classes() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { Base } from 'model'\nclass Child extends Base\n  constructor: (value) -> super(value + 1)\n  score: -> super() + 1\nexport result = new Child(40).score()",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert(
        "model",
        "class Base\n  constructor: (@value) ->\n  score: -> @value\nexport { Base }",
    );

    let exports = Context::new().run_module(&main, &loader).unwrap();
    assert_eq!(exports.get("result").and_then(Value::as_number), Some(42.));
}

#[test]
fn modules_can_export_receiver_bound_callbacks_without_exporting_receivers() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { callback } from 'counter'\nexport result = [callback(), callback()]",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert(
        "counter",
        "class Counter\n  constructor: (@value) ->\n  callback: ->\n    =>\n      @value = @value + 1\n      @value\nexport callback = new Counter(40).callback()",
    );

    let exports = Context::new().run_module(&main, &loader).unwrap();
    assert_eq!(exports.get("result").unwrap().to_string(), "[41, 42]");
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
fn modules_preserve_structured_error_values_and_trusted_labels() {
    let main = Engine::new()
        .compile_module(
            "main",
            "import { fail } from 'dependency'\nexport code = try fail() catch problem then problem.code",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert(
        "dependency",
        "fail = -> throw error('dependency.failed', 'failed', {retry: false})\nexport { fail }",
    );
    let exports = Context::new().run_module(&main, &loader).unwrap();
    assert_eq!(
        exports.get("code").and_then(Value::as_str),
        Some("dependency.failed")
    );

    let uncaught = Engine::new()
        .compile_module(
            "uncaught",
            "import { fail } from 'dependency'\nexport result = fail()",
        )
        .unwrap();
    let error = Context::new().run_module(&uncaught, &loader).unwrap_err();
    assert_eq!(error.script_error().unwrap().code(), "dependency.failed");
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("dependency")
    );
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
fn module_children_inherit_json_resource_policy() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { payload } from 'dependency'\nexport payload = payload",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert("dependency", "export payload = parse_json('[0]')");
    let limits = ResourceLimits::default().with_max_json_values(1);
    let error = Context::new()
        .with_resource_limits(limits)
        .run_module(&main, &loader)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::JsonValueCount));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("dependency")
    );
}

#[test]
fn module_children_inherit_exact_numeric_resource_policy() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { payload } from 'dependency'\nexport payload = payload",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert("dependency", "export payload = 8n");
    let limits = ResourceLimits::default().with_max_integer_bits(3);
    let error = Context::new()
        .with_resource_limits(limits)
        .run_module(&main, &loader)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::IntegerBits));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("dependency")
    );
}

#[test]
fn module_children_inherit_collection_operation_resource_policy() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { payload } from 'dependency'\nexport payload = payload",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert("dependency", "export payload = sort([3, 2, 1])");
    let limits = ResourceLimits::default().with_max_collection_operation_items(2);
    let error = Context::new()
        .with_resource_limits(limits)
        .run_module(&main, &loader)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(
        error.resource_limit(),
        Some(ResourceLimit::CollectionOperationItems)
    );
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("dependency")
    );
}

#[test]
fn module_children_inherit_general_value_resource_policy() {
    let engine = Engine::new();
    let main = engine
        .compile_module(
            "main",
            "import { payload } from 'dependency'\nexport payload = payload",
        )
        .unwrap();
    let mut loader = MemoryModuleLoader::new();
    loader.insert("dependency", "export payload = [1, 2, 3]");
    let limits = ResourceLimits::default().with_max_array_items(2);
    let error = Context::new()
        .with_resource_limits(limits)
        .run_module(&main, &loader)
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Resource);
    assert_eq!(error.resource_limit(), Some(ResourceLimit::ArrayItems));
    assert_eq!(
        error.labels()[0].span.source_name.as_deref(),
        Some("dependency")
    );
}

#[test]
fn module_directives_are_not_accepted_by_single_file_compilation() {
    let error = Engine::new()
        .compile_program_named("virtual://single.coffee", "value = 1\nexport answer = 42")
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Verify);
    assert_eq!(
        error.to_string(),
        "verify error: module directives require Engine::compile_module"
    );
    let span = &error.labels()[0].span;
    assert_eq!(span.source_name.as_deref(), Some("virtual://single.coffee"));
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
        root.join("app/main.coffee"),
        "import { double } from './lib/math'\nimport { base } from '../shared/value.coffee'\nexport result = double(base)",
    )
    .unwrap();
    fs::write(
        root.join("app/lib/math.coffee"),
        "export double = (value) -> value * 2",
    )
    .unwrap();
    fs::write(root.join("shared/value.coffee"), "export base = 21").unwrap();

    let loader = RestrictedFileModuleLoader::new(&root).unwrap();
    let source = loader.load_entry("app/main").unwrap();
    assert_eq!(source.name(), "app/main.coffee");
    let main = Engine::new()
        .compile_module(source.name(), source.source())
        .unwrap();
    let exports = Context::new().run_module(&main, &loader).unwrap();
    assert_eq!(exports.get("result").and_then(Value::as_number), Some(42.));

    let normalized = loader.load("./lib/../lib/math", "app/main.coffee").unwrap();
    assert_eq!(normalized.name(), "app/lib/math.coffee");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn restricted_file_loader_executes_explicit_litcoffee_modules() {
    let root = module_temp("literate");
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(
        root.join("app/main.litcoffee"),
        "# Literate entry\n\n    export answer = 42\n",
    )
    .unwrap();

    let loader = RestrictedFileModuleLoader::new(&root).unwrap();
    let source = loader.load_entry("app/main.litcoffee").unwrap();
    assert_eq!(source.name(), "app/main.litcoffee");
    let module = Engine::new()
        .compile_module(source.name(), source.source())
        .unwrap();
    let exports = Context::new().run_module(&module, &loader).unwrap();
    assert_eq!(exports.get("answer").and_then(Value::as_number), Some(42.));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn restricted_file_loader_rejects_ambiguous_and_escaping_paths() {
    let root = module_temp("invalid");
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(root.join("app/main.coffee"), "export value = 1").unwrap();
    let loader = RestrictedFileModuleLoader::new(&root).unwrap();

    for specifier in [
        "package",
        "/absolute",
        "./windows\\module",
        "./wrong.txt",
        "./legacy.qc",
    ] {
        let error = loader.load(specifier, "app/main.coffee").unwrap_err();
        assert_eq!(
            error.message(),
            format!("invalid module specifier: {specifier}")
        );
    }
    let error = loader.load("../../outside", "app/main.coffee").unwrap_err();
    assert_eq!(
        error.message(),
        "module path escapes configured root: ../../outside"
    );
    let error = loader.load("./missing", "app/main.coffee").unwrap_err();
    assert_eq!(error.message(), "module not found: app/missing.coffee");
    let error = loader.load("./main", "../app/main.coffee").unwrap_err();
    assert_eq!(
        error.message(),
        "invalid module referrer: ../app/main.coffee"
    );
    let error = loader.load_entry("../outside").unwrap_err();
    assert_eq!(error.message(), "invalid module entry: ../outside");
    let error = loader.load_entry("app/main.txt").unwrap_err();
    assert_eq!(error.message(), "invalid module entry: app/main.txt");
    fs::write(root.join("app/binary.coffee"), [0xff]).unwrap();
    let error = loader.load_entry("app/binary").unwrap_err();
    assert_eq!(
        error.message(),
        "module source is not readable UTF-8: app/binary.coffee"
    );
    fs::create_dir(root.join("app/directory.coffee")).unwrap();
    let error = loader.load_entry("app/directory").unwrap_err();
    assert_eq!(
        error.message(),
        "invalid module target: app/directory.coffee"
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn restricted_file_loader_canonicalizes_aliases_and_rejects_symlink_escape() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let base = module_temp("symlink");
    let root = base.join("root");
    fs::create_dir_all(root.join("real")).unwrap();
    fs::write(root.join("real/module.coffee"), "export value = 42").unwrap();
    fs::write(base.join("outside.coffee"), "export value = 0").unwrap();
    fs::write(root.join("real:module.coffee"), "export value = 1").unwrap();
    fs::write(root.join("real\\module.coffee"), "export value = 2").unwrap();
    symlink("real/module.coffee", root.join("alias.coffee")).unwrap();
    symlink(base.join("outside.coffee"), root.join("escape.coffee")).unwrap();
    symlink("real:module.coffee", root.join("colon-alias.coffee")).unwrap();
    symlink("real\\module.coffee", root.join("backslash-alias.coffee")).unwrap();

    let loader = RestrictedFileModuleLoader::new(&root).unwrap();
    let alias = loader.load_entry("alias").unwrap();
    assert_eq!(alias.name(), "real/module.coffee");
    let error = loader.load_entry("escape").unwrap_err();
    assert_eq!(
        error.message(),
        "module path escapes configured root: escape.coffee"
    );
    for alias in ["colon-alias", "backslash-alias"] {
        let error = loader.load_entry(alias).unwrap_err();
        assert_eq!(
            error.message(),
            format!("invalid module target: {alias}.coffee")
        );
    }
    let locked = root.join("locked.coffee");
    fs::write(&locked, "export value = 3").unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let error = loader.load_entry("locked").unwrap_err();
    assert_eq!(
        error.message(),
        "module source is not readable: locked.coffee"
    );
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o600)).unwrap();
    let _ = fs::remove_dir_all(base);
}

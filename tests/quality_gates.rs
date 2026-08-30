use std::{fs, path::Path};

use quickcoffee::Context;

fn repository_file(path: impl AsRef<Path>) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .expect("repository quality-gate file is readable")
}

#[test]
fn vm_fuzz_target_is_bounded_seeded_and_part_of_smoke() {
    let manifest = repository_file("fuzz/Cargo.toml");
    assert!(manifest.contains("name = \"vm\""));
    assert!(manifest.contains("path = \"fuzz_targets/vm.rs\""));

    let target = repository_file("fuzz/fuzz_targets/vm.rs");
    for boundary in [
        "MAX_INPUT_BYTES",
        "with_fuel(50_000)",
        "with_max_call_depth(64)",
        "with_max_string_bytes",
        "with_max_array_items",
        "with_max_map_entries",
        "with_max_retained_managed_objects",
        "with_max_retained_managed_bytes",
    ] {
        assert!(
            target.contains(boundary),
            "missing VM fuzz boundary: {boundary}"
        );
    }
    assert!(target.contains("eval_named(\"fuzz-input.coffee\""));

    for seed in [
        "fuzz/seed_corpus/vm/functions.coffee",
        "fuzz/seed_corpus/vm/classes-errors.coffee",
        "fuzz/seed_corpus/vm/containers-json.coffee",
    ] {
        let source = repository_file(seed);
        assert!(!source.trim().is_empty(), "empty seed: {seed}");
        Context::new()
            .with_fuel(100_000)
            .eval_named(seed, &source)
            .unwrap_or_else(|error| panic!("VM seed must execute successfully: {seed}: {error}"));
    }

    let makefile = repository_file("Makefile");
    assert!(makefile.contains("mkdir -p fuzz/corpus/parser fuzz/corpus/verifier fuzz/corpus/vm"));
    for target in ["parser", "verifier", "vm"] {
        assert!(
            makefile.contains(&format!(
                "fuzz run {target} fuzz/corpus/{target} fuzz/seed_corpus/{target}"
            )),
            "{target} smoke must consume tracked seeds through a writable ignored corpus"
        );
    }
    assert_eq!(
        makefile
            .matches("-runs=1024 -seed=1 -max_len=16384")
            .count(),
        3
    );
    assert_eq!(makefile.matches("-detect_leaks=0").count(), 1);
    assert_eq!(makefile.matches("ASAN_OPTIONS=detect_leaks=0").count(), 1);
    assert!(makefile.contains(
        "ASAN_OPTIONS=detect_leaks=0 cargo +nightly-2026-08-20 fuzz run vm fuzz/corpus/vm fuzz/seed_corpus/vm -- -runs=1024 -seed=1 -max_len=16384 -detect_leaks=0"
    ));
}

#[test]
fn ci_runs_for_pull_requests_and_main_pushes_without_duplicate_feature_pushes() {
    let ci = repository_file(".github/workflows/ci.yml");
    let (_, after_on) = ci
        .split_once("on:\n")
        .expect("CI workflow declares event triggers");
    let (triggers, _) = after_on
        .split_once("\npermissions:")
        .expect("CI workflow declares permissions after its triggers");

    assert_eq!(
        triggers,
        "  push:\n    branches:\n      - main\n  pull_request:\n"
    );
    assert!(ci.contains("toolchain: [\"1.85.0\", stable]"));
    assert!(ci.contains("- run: make docs && make check"));
    assert!(ci.contains("- run: git diff --exit-code"));
    assert!(ci.contains("- run: test -z \"$(git status --short)\""));
}

#[test]
fn dependency_and_miri_workflows_keep_their_pinned_boundaries() {
    let toolchain = repository_file("fuzz/rust-toolchain.toml");
    assert!(toolchain.contains("nightly-2026-08-20"));

    let audit = repository_file(".github/workflows/security.yml");
    assert!(audit.contains("cargo-audit --version 0.22.2 --locked"));
    assert!(audit.contains("make dependency-audit"));
    assert!(audit.contains("schedule:"));
    assert!(audit.contains("workflow_dispatch:"));
    assert!(!audit.contains("--ignore"));

    let makefile = repository_file("Makefile");
    assert!(makefile.contains("cargo audit --file Cargo.lock"));
    assert!(makefile.contains("cargo audit --file fuzz/Cargo.lock"));
    assert!(makefile.contains("MIRIFLAGS=\"-Zmiri-ignore-leaks\""));
    assert!(makefile.contains("cargo +nightly-2026-08-20 miri test --lib"));
    assert!(
        makefile.contains(
            "--skip json::tests::malformed_numbers_nesting_and_size_limits_fail_atomically"
        )
    );
    assert!(!makefile.contains("-Zmiri-disable-isolation"));

    let fuzz = repository_file(".github/workflows/fuzz.yml");
    assert_eq!(fuzz.matches("nightly-2026-08-20").count(), 3);
    assert!(fuzz.contains("components: miri"));
    assert!(fuzz.contains("make miri-smoke"));
    assert!(fuzz.contains("actions/upload-artifact@v7"));
}

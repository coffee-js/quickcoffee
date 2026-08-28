.PHONY: fmt test release-test examples package-metadata package release-tool-check qbench-check fuzz-smoke miri-smoke dependency-audit clippy api-doc docs docs-html doc-check check bench qbench

fmt:
	cargo fmt --check

test:
	cargo build --locked --bins
	cargo test --locked

release-test:
	cargo test --locked --release

examples:
	cargo test --locked --examples

package-metadata:
	cargo metadata --locked --no-deps --format-version 1 >/dev/null

package:
	cargo publish --dry-run --locked --allow-dirty

release-tool-check:
	python3 scripts/test_release.py

clippy:
	cargo clippy --locked --all-targets -- -D warnings

api-doc:
	RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc --locked --no-deps

doc-check:
	cargo run --locked --quiet --bin qdocco -- --check manuals/manual.zh-CN.litcoffee
	cargo run --locked --quiet --bin qdocco -- --check manuals/manual.classical-zh.litcoffee
	cargo run --locked --quiet --bin qdocco -- --check manuals/manual.en.litcoffee
	cargo run --locked --quiet --bin qdocco -- --check manuals/manual.latin.litcoffee
	cargo run --locked --quiet --bin qdocco -- --check manuals/manual.devanagari-sa.litcoffee

docs: doc-check
	cargo run --locked --quiet --bin qdocco -- --markdown manuals/manual.zh-CN.litcoffee -o docs/manual.zh-CN.md
	cargo run --locked --quiet --bin qdocco -- --markdown manuals/manual.classical-zh.litcoffee -o docs/manual.classical-zh.md
	cargo run --locked --quiet --bin qdocco -- --markdown manuals/manual.en.litcoffee -o docs/manual.en.md
	cargo run --locked --quiet --bin qdocco -- --markdown manuals/manual.latin.litcoffee -o docs/manual.latin.md
	cargo run --locked --quiet --bin qdocco -- --markdown manuals/manual.devanagari-sa.litcoffee -o docs/manual.devanagari-sa.md

docs-html: doc-check
	mkdir -p target/manuals
	cargo run --locked --quiet --bin qdocco -- manuals/manual.zh-CN.litcoffee -o target/manuals/manual.zh-CN.html
	cargo run --locked --quiet --bin qdocco -- manuals/manual.classical-zh.litcoffee -o target/manuals/manual.classical-zh.html
	cargo run --locked --quiet --bin qdocco -- manuals/manual.en.litcoffee -o target/manuals/manual.en.html
	cargo run --locked --quiet --bin qdocco -- manuals/manual.latin.litcoffee -o target/manuals/manual.latin.html
	cargo run --locked --quiet --bin qdocco -- manuals/manual.devanagari-sa.litcoffee -o target/manuals/manual.devanagari-sa.html

check: fmt test release-test examples package-metadata package release-tool-check qbench-check clippy api-doc doc-check

bench:
	cargo bench --locked --bench core

qbench:
	cargo run --locked --release --bin qbench -- --json

qbench-check:
	cargo run --locked --quiet --release --bin qbench -- --json --iterations 1 --repeat 3 >/dev/null

fuzz-smoke:
	mkdir -p fuzz/corpus/parser fuzz/corpus/verifier fuzz/corpus/vm
	cargo +nightly-2026-08-20 fuzz run parser fuzz/corpus/parser fuzz/seed_corpus/parser -- -runs=1024 -seed=1 -max_len=16384
	cargo +nightly-2026-08-20 fuzz run verifier fuzz/corpus/verifier fuzz/seed_corpus/verifier -- -runs=1024 -seed=1 -max_len=16384
	cargo +nightly-2026-08-20 fuzz run vm fuzz/corpus/vm fuzz/seed_corpus/vm -- -runs=1024 -seed=1 -max_len=16384

miri-smoke:
	MIRIFLAGS="-Zmiri-ignore-leaks" cargo +nightly-2026-08-20 miri test --lib -- --skip json::tests::malformed_numbers_nesting_and_size_limits_fail_atomically

dependency-audit:
	cargo audit --file Cargo.lock
	cargo audit --file fuzz/Cargo.lock

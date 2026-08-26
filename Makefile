.PHONY: fmt test release-test examples package-metadata package qbench-check fuzz-smoke clippy api-doc docs doc-check check bench qbench

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

clippy:
	cargo clippy --locked --all-targets -- -D warnings

api-doc:
	RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc --locked --no-deps

doc-check:
	cargo run --locked --quiet --bin qdocco -- --check manuals/manual.zh-CN.qc
	cargo run --locked --quiet --bin qdocco -- --check manuals/manual.classical-zh.qc
	cargo run --locked --quiet --bin qdocco -- --check manuals/manual.en.qc
	cargo run --locked --quiet --bin qdocco -- --check manuals/manual.latin.qc
	cargo run --locked --quiet --bin qdocco -- --check manuals/manual.devanagari-sa.qc

docs: doc-check
	cargo run --locked --quiet --bin qdocco -- manuals/manual.zh-CN.qc -o docs/manual.zh-CN.html
	cargo run --locked --quiet --bin qdocco -- manuals/manual.classical-zh.qc -o docs/manual.classical-zh.html
	cargo run --locked --quiet --bin qdocco -- manuals/manual.en.qc -o docs/manual.en.html
	cargo run --locked --quiet --bin qdocco -- manuals/manual.latin.qc -o docs/manual.latin.html
	cargo run --locked --quiet --bin qdocco -- manuals/manual.devanagari-sa.qc -o docs/manual.devanagari-sa.html
	cargo run --locked --quiet --bin qdocco -- --markdown manuals/manual.zh-CN.qc -o docs/manual.zh-CN.md
	cargo run --locked --quiet --bin qdocco -- --markdown manuals/manual.classical-zh.qc -o docs/manual.classical-zh.md
	cargo run --locked --quiet --bin qdocco -- --markdown manuals/manual.en.qc -o docs/manual.en.md
	cargo run --locked --quiet --bin qdocco -- --markdown manuals/manual.latin.qc -o docs/manual.latin.md
	cargo run --locked --quiet --bin qdocco -- --markdown manuals/manual.devanagari-sa.qc -o docs/manual.devanagari-sa.md

check: fmt test release-test examples package-metadata package qbench-check clippy api-doc doc-check

bench:
	cargo bench --locked --bench core

qbench:
	cargo run --locked --release --bin qbench -- --json

qbench-check:
	cargo run --locked --quiet --release --bin qbench -- --json --iterations 1 --repeat 3 >/dev/null

fuzz-smoke:
	cargo +nightly-2025-03-28 fuzz run parser -- -runs=1024 -seed=1
	cargo +nightly-2025-03-28 fuzz run verifier -- -runs=1024 -seed=1

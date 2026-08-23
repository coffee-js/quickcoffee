.PHONY: fmt test examples clippy api-doc docs doc-check check bench qbench

fmt:
	cargo fmt --check

test:
	cargo test

examples:
	cargo test --examples
	test "$$(cargo run --quiet --example embed)" = "84"

clippy:
	cargo clippy --all-targets -- -D warnings

api-doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps

doc-check:
	cargo run --quiet --bin qdocco -- --check manuals/manual.zh-CN.qc
	cargo run --quiet --bin qdocco -- --check manuals/manual.classical-zh.qc
	cargo run --quiet --bin qdocco -- --check manuals/manual.en.qc
	cargo run --quiet --bin qdocco -- --check manuals/manual.latin.qc
	cargo run --quiet --bin qdocco -- --check manuals/manual.devanagari-sa.qc

docs: doc-check
	cargo run --quiet --bin qdocco -- manuals/manual.zh-CN.qc -o docs/manual.zh-CN.html
	cargo run --quiet --bin qdocco -- manuals/manual.classical-zh.qc -o docs/manual.classical-zh.html
	cargo run --quiet --bin qdocco -- manuals/manual.en.qc -o docs/manual.en.html
	cargo run --quiet --bin qdocco -- manuals/manual.latin.qc -o docs/manual.latin.html
	cargo run --quiet --bin qdocco -- manuals/manual.devanagari-sa.qc -o docs/manual.devanagari-sa.html

check: fmt test examples clippy api-doc doc-check

bench:
	cargo bench --bench core

qbench:
	cargo run --release --bin qbench -- --json

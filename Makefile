.PHONY: fmt test examples package-metadata clippy api-doc docs doc-check check bench qbench

fmt:
	cargo fmt --check

test:
	cargo test --locked

examples:
	cargo test --locked --examples

package-metadata:
	cargo metadata --locked --no-deps --format-version 1 >/dev/null
	cargo metadata --locked --no-deps --format-version 1 | grep -Eq '"repository":"[^"]+"'
	cargo metadata --locked --no-deps --format-version 1 | grep -Eq '"homepage":"[^"]+"'
	cargo metadata --locked --no-deps --format-version 1 | grep -Eq '"documentation":"[^"]+"'
	cargo metadata --locked --no-deps --format-version 1 | grep -Eq '"readme":"[^"]+"'
	cargo metadata --locked --no-deps --format-version 1 | grep -Eq '"keywords":\[[^]]*"[^"]+"'
	cargo metadata --locked --no-deps --format-version 1 | grep -Eq '"categories":\[[^]]*"[^"]+"'

clippy:
	cargo clippy --locked --all-targets -- -D warnings

api-doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps

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

check: fmt test examples package-metadata clippy api-doc doc-check

bench:
	cargo bench --locked --bench core

qbench:
	cargo run --locked --release --bin qbench -- --json

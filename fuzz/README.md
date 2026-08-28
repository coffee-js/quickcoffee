# QuickCoffee fuzzing

`fuzz/` is a separate cargo-fuzz package. It is not part of the published
crate or the Rust 1.85/stable workspace checks. The pinned nightly in
`rust-toolchain.toml` and cargo-fuzz 0.13.2 apply only here.

Run one bounded target locally. The ignored writable corpus comes first and
the tracked seed corpus second, so a smoke run cannot modify reviewed seeds:

```sh
cargo +nightly-2026-08-20 fuzz run parser fuzz/corpus/parser fuzz/seed_corpus/parser -- -max_total_time=30 -seed=1 -max_len=16384
cargo +nightly-2026-08-20 fuzz run verifier fuzz/corpus/verifier fuzz/seed_corpus/verifier -- -max_total_time=30 -seed=1 -max_len=16384
cargo +nightly-2026-08-20 fuzz run vm fuzz/corpus/vm fuzz/seed_corpus/vm -- -max_total_time=30 -seed=1 -max_len=16384
```

`make fuzz-smoke` uses the same deterministic seed and bounded input for all
three targets. The VM harness also bounds fuel, call depth, general values,
JSON, exact numbers, and retained state. A fuzz-discovered crash is not
accepted as an ordinary language error: minimize it with `cargo fuzz tmin`,
add a deterministic regression test, and then decide whether the minimized
input remains in the corpus. Harnesses do not grant scripts filesystem,
network, clock, or other host capabilities.

Tracked `seed_corpus/` files document representative parser recovery and
verifier inputs. cargo-fuzz writes evolving coverage corpus files below the
ignored `corpus/` directory; do not commit them without minimizing and adding
the corresponding deterministic regression test.

The scheduled/manual workflow also runs `make miri-smoke` with the same pinned
nightly. Miri keeps host isolation enabled. It ignores expected process-lifetime
`Rc`/thread-local leaks and skips only the deliberately high-complexity JSON
stress; the remaining library tests are interpreted. Install and prepare it
locally with:

```sh
rustup toolchain install nightly-2026-08-20 --profile minimal --component miri
cargo +nightly-2026-08-20 miri setup
make miri-smoke
```

`make dependency-audit` checks both `Cargo.lock` and `fuzz/Cargo.lock`. CI pins
`cargo-audit 0.22.2`; advisories fail the job and are not silently ignored.

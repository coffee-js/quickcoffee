# QuickCoffee fuzzing

`fuzz/` is a separate cargo-fuzz package. It is not part of the published
crate or the Rust 1.85/stable workspace checks. The pinned nightly in
`rust-toolchain.toml` and cargo-fuzz 0.13.2 apply only here.

Run one bounded target locally:

```sh
cargo +nightly-2025-03-28 fuzz run parser -- -max_total_time=30 -seed=1
cargo +nightly-2025-03-28 fuzz run verifier -- -max_total_time=30 -seed=1
```

`make fuzz-smoke` uses the same deterministic seed with a short duration. A
fuzz-discovered crash is not accepted as an ordinary language error: minimize
it with `cargo fuzz tmin`, add a deterministic regression test, and then
decide whether the minimized input remains in the corpus. Harnesses do not
grant scripts filesystem, network, clock, or other host capabilities.

Tracked `seed_corpus/` files document representative parser recovery and
verifier inputs. cargo-fuzz writes evolving coverage corpus files below the
ignored `corpus/` directory; do not commit them without minimizing and adding
the corresponding deterministic regression test.

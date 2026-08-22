use quickcoffee::{Chunk, Constant, Context, Instruction, Pattern, Value, compile};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn malformed_source_is_reported_without_panicking() {
    let corpus = [
        "",
        "(",
        ")",
        "[1..]",
        "[1... ]",
        "'unterminated",
        "\"unterminated",
        "\"#{\"",
        "if true",
        "if true\n   1\n  2",
        "for x in",
        "for x, y in [1] then x",
        "for own key of {a: 1} then key",
        "try 1",
        "catch error then 1",
        "(head..., tail) -> head",
        "x...",
        "@field",
        "`host()`",
        "\u{0}",
        "变量 = [1..3]\n变量[",
    ];
    for source in corpus {
        let result = catch_unwind(AssertUnwindSafe(|| compile(source)));
        assert!(result.is_ok(), "compile panicked for {source:?}");
    }
}

#[test]
fn deterministic_syntax_stress_corpus_never_panics_the_compiler() {
    const FRAGMENTS: &[&str] = &[
        "a",
        "स्थित",
        "1",
        "nil",
        "true",
        " + ",
        " ? ",
        " not in ",
        " and ",
        "[",
        "]",
        "{",
        "}",
        "(",
        ")",
        "...",
        "..",
        "->",
        "=>",
        "\n",
        "\n  ",
        "\t",
        "'unterminated",
        "\"#{",
        "### comment\n",
        "###\n",
        "@",
        "`",
        "\u{0}",
    ];
    for seed in 0_u64..1_024 {
        let mut state = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut source = String::new();
        for _ in 0..(8 + seed % 25) {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            source.push_str(FRAGMENTS[(state as usize) % FRAGMENTS.len()]);
        }
        let result = catch_unwind(AssertUnwindSafe(|| compile(&source)));
        assert!(
            result.is_ok(),
            "compile panicked for seed {seed}: {source:?}"
        );
    }
}

#[test]
fn runtime_errors_return_errors_without_panicking() {
    for source in ["1 / 'x'", "[0...1000001]", "throw 'x'", "2 in {x: 2}"] {
        let result = catch_unwind(AssertUnwindSafe(|| Context::new().eval(source)));
        assert!(result.is_ok(), "evaluation panicked for {source:?}");
        assert!(result.expect("no panic").is_err());
    }
}

#[test]
fn malformed_bytecode_verification_never_panics() {
    for seed in 0_u64..2_048 {
        let mut state = seed.wrapping_add(0x517c_c1b7_2722_0a95);
        let mut code = Vec::new();
        for _ in 0..(state as usize % 24) {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let offset = state as i32;
            code.push(match state % 16 {
                0 => Instruction::Constant(0),
                1 => Instruction::Pop,
                2 => Instruction::Dup,
                3 => Instruction::Swap,
                4 => Instruction::Jump(offset),
                5 => Instruction::JumpIfFalse(offset),
                6 => Instruction::JumpIfNil(offset),
                7 => Instruction::Try {
                    catch: offset,
                    name: "error".into(),
                },
                8 => Instruction::EndTry,
                9 => Instruction::IterStartEnumerable,
                10 => Instruction::IterNext {
                    patterns: vec![Pattern::Bind("item".into())],
                    end: offset,
                },
                11 => Instruction::IterEnd,
                12 => Instruction::MakeArray((state % 4) as usize),
                13 => Instruction::Call((state % 4) as usize),
                14 => Instruction::MakeMap(vec!["key".into()]),
                _ => Instruction::Return,
            });
        }
        let chunk = Chunk {
            constants: vec![Constant::Value(Value::Nil)],
            code,
        };
        let result = catch_unwind(AssertUnwindSafe(|| chunk.verify()));
        assert!(result.is_ok(), "verifier panicked for seed {seed}");
    }
}

use quickcoffee::{
    Chunk, Constant, Context, Engine, ErrorKind, Instruction, Pattern, Program, Value, compile,
};
use std::{cell::Cell, rc::Rc};

fn eval(source: &str) -> Value {
    Context::new().eval(source).unwrap()
}
#[test]
fn arithmetic_precedence_and_arrays() {
    assert_eq!(eval("[1 + 2 * 3, 2 ** 3]").to_string(), "[7, 8]");
    let chunk = compile("1 + 2 * 3").unwrap();
    assert!(!chunk.disassemble().contains("Add"));
    assert!(!chunk.disassemble().contains("Mul"));
    assert!(chunk.verify().is_ok());
    assert_eq!(eval("if true then 2 else missing").as_number(), Some(2.));
    assert!(Context::new().eval("1.5 & 1").is_err());
}
#[test]
fn implicit_calls_accept_single_nested_and_comma_separated_arguments() {
    assert_eq!(
        eval("add = (left, right) -> left + right\nadd 20, 22").as_number(),
        Some(42.)
    );
    assert_eq!(
        eval("increment = (value) -> value + 1\nincrement 2 * 3").as_number(),
        Some(7.)
    );
    assert_eq!(
        eval("increment = (value) -> value + 1\ndouble = (value) -> value * 2\ndouble increment 2")
            .as_number(),
        Some(6.)
    );
    assert_eq!(eval("len [1, 2, 3]").as_number(), Some(3.));
    assert!(Context::new().eval("add(20 22)").is_err());
}
#[test]
fn embedding_execution_stats_cover_success_runtime_error_and_fuel() {
    let mut cx = Context::new().with_fuel(100);
    assert_eq!(cx.eval("1 + 2").unwrap().as_number(), Some(3.));
    let success = cx.last_execution();
    assert!(success.instructions > 0);
    assert_eq!(success.instructions + success.fuel_remaining, 100);
    assert!(cx.eval("(").is_err());
    assert_eq!(cx.last_execution(), success);

    let error = cx.eval("unknown_name").unwrap_err();
    assert!(error.message().contains("unknown name"));
    let failed = cx.last_execution();
    assert!(failed.instructions > 0);
    assert!(failed.instructions + failed.fuel_remaining <= 100);

    let invalid_program = Program::from(Chunk::default());
    assert!(cx.run_program(&invalid_program).is_err());
    assert_eq!(cx.last_execution(), failed);

    let mut exhausted = cx.with_fuel(5);
    assert!(exhausted.eval("while true then 1").is_err());
    let fuel = exhausted.last_execution();
    assert_eq!(fuel.instructions, 5);
    assert_eq!(fuel.fuel_remaining, 0);
}
#[test]
fn explicit_operator_line_continuation_preserves_expression_and_layout() {
    assert_eq!(eval("value = 1 +\n  2 * 3\nvalue").as_number(), Some(7.));
    assert_eq!(
        eval("if true\n  value = 1 +\n    2\n  value").as_number(),
        Some(3.)
    );
    assert_eq!(
        eval("value = 1 +\n  2\nnext = value + 3\nnext").as_number(),
        Some(6.)
    );
    assert_eq!(eval("value = nil\nvalue?\n42").as_number(), Some(42.));
    let chunk = compile("value = 1 +\n  2").unwrap();
    assert!(chunk.verify().is_ok());
    let error = compile("value = 1 +\n").unwrap_err();
    assert_eq!(error.position().map(|position| position.line), Some(2));
}
#[test]
fn ordinary_quoted_strings_join_physical_lines_without_leaking_layout() {
    assert_eq!(eval("\"hello\n  world\"").to_string(), "hello world");
    assert_eq!(eval("'hello\n  world'").to_string(), "hello world");
    assert_eq!(eval("\"answer #{1 +\n  1}\"").to_string(), "answer 2");
    assert_eq!(
        eval(
            r#""hello\
  world""#
        )
        .to_string(),
        "helloworld"
    );
    assert_eq!(
        eval("if true\n  text = \"hello\n    world\"\n  text").to_string(),
        "hello world"
    );
    assert_eq!(
        eval("len(\"left\") +\n  len(\"right\")").as_number(),
        Some(9.)
    );
    let chunk = compile("message = \"hello\n  world\"\nmessage").unwrap();
    assert!(chunk.verify().is_ok());
    let error = compile("\"unfinished\nnext").unwrap_err();
    assert_eq!(error.position().map(|position| position.line), Some(1));
}
#[test]
fn quoted_strings_decode_common_hex_and_unicode_escapes() {
    assert_eq!(
        eval(r#""\0\b\f\n\r\t\v\x41\u0042\u{1F600}""#).as_str(),
        Some("\0\u{0008}\u{000c}\n\r\t\u{000b}AB😀")
    );
    assert_eq!(eval("'\\n\\u0041'").as_str(), Some("\nA"));
    for source in [
        r#""\x4""#,
        r#""\u12""#,
        r#""\u{}""#,
        r#""\u{110000}""#,
        r#""\q""#,
    ] {
        let error = Context::new().eval(source).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Parse);
    }
    let error = Context::new().eval("value = 1\n\"\\q\"").unwrap_err();
    assert_eq!(error.position().map(|position| position.line), Some(2));
}
#[test]
fn multiline_arrays_and_maps_accept_line_separators_without_commas() {
    assert_eq!(
        eval("values = [\n  1\n  2\n  3\n]\nvalues").to_string(),
        "[1, 2, 3]"
    );
    assert_eq!(
        eval("record = {\n  first: 1\n  nested: [\n    2\n    3\n  ]\n}\nrecord.first + record.nested[1]").as_number(),
        Some(4.)
    );
    assert_eq!(eval("[1,\n 2,\n]").to_string(), "[1, 2]");
    assert!(Context::new().eval("sum(\n 1\n 2\n)").is_err());
    assert!(Context::new().eval("[1}").is_err());
}
#[test]
fn indented_map_literals_lower_recursively_without_changing_assignment_continuation() {
    assert_eq!(
        eval("record =\n  first: 1\n  nested:\n    second: 2\n  third: 3\nrecord.nested.second + record.third").as_number(),
        Some(5.)
    );
    assert_eq!(eval("value =\n  1 + 2\nvalue").as_number(), Some(3.));
    assert_eq!(
        eval("make = ->\n  record =\n    answer: 42\n  record.answer\nmake()").as_number(),
        Some(42.)
    );
}
#[test]
fn floor_division_and_dividend_dependent_modulo_are_strict_numeric_operators() {
    assert_eq!(
        eval("[-7 // 5, -7 % 5, -7 %% 5, 7 // -5, 7 %% -5]").to_string(),
        "[-2, -2, 3, -2, -3]"
    );
    assert_eq!(
        eval("value = 7\nvalue //= 2\nvalue %%= 5\nvalue").as_number(),
        Some(3.)
    );
    let chunk = compile("-7 // 5 + 7 %% 5").unwrap();
    assert!(chunk.verify().is_ok());
    assert!(Context::new().eval("'x' // 2").is_err());
    assert!(
        Context::new()
            .eval("record = {value: 1}\nrecord.value //= 2")
            .is_err()
    );
}
#[test]
fn strict_bitwise_operators_use_signed_32_bit_numbers() {
    assert_eq!(
        eval("[5 & 3, 5 | 2, 5 ^ 1, ~1, 1 << 3, -8 >> 2, -1 >>> 1]").to_string(),
        "[1, 7, 4, -2, 8, -2, 2147483647]"
    );
    assert_eq!(
        eval("value = 5\nvalue &= 3\nvalue |= 8\nvalue ^= 2\nvalue <<= 1\nvalue >>= 2\nvalue >>>= 1\nvalue")
            .as_number(),
        Some(2.)
    );
    assert_eq!(eval("1 | 2 & 3").as_number(), Some(3.));
    let chunk = compile("value = 1\n~value + (value << 2)").unwrap();
    assert!(chunk.verify().is_ok());
    assert!(chunk.disassemble().contains("BitNot"));
    for source in [
        "1.5 & 1",
        "2147483648 & 1",
        "1 << 32",
        "'x' | 1",
        "record = {value: 1}\nrecord.value &= 1",
    ] {
        assert!(Context::new().eval(source).is_err(), "accepted {source:?}");
    }
}
#[test]
fn numeric_literals_support_radix_and_exponent_forms() {
    assert_eq!(eval("0xff + 0b10 + 0o7 + 1e1").as_number(), Some(274.));
    assert!(compile("0xff + 1e-2").unwrap().verify().is_ok());
    let error = compile("0x\n").unwrap_err();
    assert_eq!(error.position().map(|position| position.line), Some(1));
}
#[test]
fn assignments_and_if_expression() {
    assert_eq!(
        eval("answer = if 2 < 3 then 42 else 0\nanswer").as_number(),
        Some(42.)
    );
}
#[test]
fn compound_assignments_update_names_with_strict_arithmetic() {
    assert_eq!(
        eval("value = 2\nvalue += 3\nvalue *= 4\nvalue -= 5\nvalue /= 3\nvalue %= 3\nvalue **= 2\nvalue")
            .as_number(),
        Some(4.)
    );
    let chunk = compile("value = 2\nvalue += 3\nvalue").unwrap();
    assert!(chunk.verify().is_ok());
    assert!(
        Context::new()
            .eval("record = {value: 1}\nrecord.value += 1")
            .is_err()
    );
    assert!(Context::new().eval("items = [1]\nitems[0] += 1").is_err());
}
#[test]
fn increment_and_decrement_update_names_with_prefix_and_postfix_values() {
    assert_eq!(
        eval("value = 2\nprefix = ++value\npostfix = value++\nvalue--\n[prefix, postfix, value]")
            .to_string(),
        "[3, 3, 3]"
    );
    assert_eq!(
        eval("value = 2\nresult = value++ + ++value\n[result, value]").to_string(),
        "[6, 4]"
    );
    assert_eq!(eval("value = 2\n--value\nvalue").as_number(), Some(1.));
    let chunk = compile("value = 1\nvalue++\n++value").unwrap();
    assert!(chunk.verify().is_ok());
    assert!(Context::new().eval("++missing").is_err());
    assert!(
        Context::new()
            .eval("record = {value: 1}\nrecord.value++")
            .is_err()
    );
    assert!(Context::new().eval("items = [1]\nitems[0]--").is_err());
    assert!(Context::new().eval("++(value)").is_err());
}
#[test]
fn embedding_errors_have_stable_kinds_and_details() {
    let parse = compile("(").unwrap_err();
    assert_eq!(parse.kind(), ErrorKind::Parse);
    assert!(!parse.message().is_empty());

    let verify = Context::new()
        .run(Chunk {
            constants: vec![],
            code: vec![Instruction::Return],
        })
        .unwrap_err();
    assert_eq!(verify.kind(), ErrorKind::Verify);
    assert!(verify.message().contains("stack"));

    let runtime = Context::new().eval("1 / 'x'").unwrap_err();
    assert_eq!(runtime.kind(), ErrorKind::Runtime);
    assert!(!runtime.message().is_empty());
    assert_eq!(
        runtime.to_string(),
        format!("runtime error: {}", runtime.message())
    );
}
#[test]
fn parse_errors_expose_source_lines_and_display_them() {
    let error = compile("value = 1\nif true\n  1 2").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Parse);
    assert_eq!(error.position().map(|position| position.line), Some(3));
    assert!(error.to_string().contains("parse error (line 3):"));
}
#[test]
fn lexical_errors_expose_source_lines() {
    let error = compile("value = 1\n@").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Parse);
    assert_eq!(error.message(), "unexpected character '@'");
    assert_eq!(error.position().map(|position| position.line), Some(2));
    assert!(error.to_string().contains("parse error (line 2):"));
}
#[test]
fn unicode_xid_identifiers_support_combining_marks() {
    assert_eq!(eval("स्थित = 40\nस्थित + 2").as_number(), Some(42.));
    assert!(Context::new().eval("\u{94b}value = 1").is_err());
    assert!(compile("स्थित = 42\nस्थित").unwrap().verify().is_ok());
}
#[test]
fn unless_and_postfix_conditions_compile_to_normal_branches() {
    assert_eq!(eval("unless false then 42 else 0").as_number(), Some(42.));
    assert_eq!(eval("42 if true").as_number(), Some(42.));
    assert!(matches!(eval("42 unless true"), Value::Nil));
}
#[test]
fn switch_when_evaluates_its_subject_once_and_has_no_fallthrough() {
    assert_eq!(
        eval("switch 2\n  when 1 then 'one'\n  when 2 then 'two'\n  else 'other'").as_str(),
        Some("two")
    );
    assert_eq!(
        eval("switch 9\n  when 1 then 'one'\n  else\n    value = 'fallback'\n    value").as_str(),
        Some("fallback")
    );
    assert_eq!(
        eval("switch 2\n  when 1, 2 then 'small'\n  else 'other'").as_str(),
        Some("small")
    );
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let mut cx = Context::new();
    cx.add_native("tick", move |_| {
        observed.set(observed.get() + 1);
        Ok(Value::Number(2.))
    });
    assert_eq!(
        cx.eval("switch tick()\n  when 1 then 0\n  when 2 then 42")
            .unwrap()
            .as_number(),
        Some(42.)
    );
    assert_eq!(calls.get(), 1);
}
#[test]
fn try_catch_finally_handles_vm_and_user_errors() {
    assert_eq!(
        eval("try\n  throw 'bad'\ncatch error\n  error\nfinally\n  cleanup = true").as_str(),
        Some("runtime error: thrown: bad")
    );
    assert_eq!(
        eval("try 1 / 'x' catch error then 'recovered'").as_str(),
        Some("recovered")
    );
    assert_eq!(
        eval("bad = () -> throw 'nested'\ntry bad() catch error then error").as_str(),
        Some("runtime error: thrown: nested")
    );
    let mut cx = Context::new();
    cx.eval("try 42 catch error then 0 finally cleanup = 7")
        .unwrap();
    assert_eq!(cx.eval("cleanup").unwrap().as_number(), Some(7.));
    assert!(
        Context::new()
            .eval("try throw 'first' catch error then throw 'second'")
            .is_err()
    );
}
#[test]
fn return_exits_functions_loops_and_protected_regions_cleanly() {
    assert_eq!(
        eval("choose = (n) ->\n  if n == 0\n    return 40\n  n + 1\n[choose(0), choose(41)]")
            .to_string(),
        "[40, 42]"
    );
    assert_eq!(
        eval("first_even = (items) ->\n  for n in items then if n % 2 == 0 then return n\n  nil\nfirst_even([1, 3, 8])")
            .as_number(),
        Some(8.)
    );
    assert_eq!(
        eval("caught = -> try throw 'bad' catch error then return 7 finally 0\ncaught()")
            .as_number(),
        Some(7.)
    );
    assert_eq!(
        eval("overridden = -> try return 1 catch error then 2 finally return 3\noverridden()")
            .as_number(),
        Some(3.)
    );
    assert!(matches!(eval("empty = -> return\nempty()"), Value::Nil));
    assert_eq!(
        eval("nested = ->\n  try\n    try return 1 catch error then 2 finally return 3\n  catch error\n    4\n  finally\n    return 5\nnested()")
            .as_number(),
        Some(5.)
    );
    assert!(
        Context::new()
            .eval("cleanup = -> try return 1 catch error then 2 finally throw 'cleanup'\ncleanup()")
            .is_err()
    );
    assert!(Context::new().eval("return 1").is_err());
    assert!(compile("f = -> return 42\nf()").unwrap().verify().is_ok());
}
#[test]
fn indentation_blocks_support_multi_statement_control_flow_and_functions() {
    assert_eq!(
        eval("if true\n  value = 40\n  value + 2\nelse\n  0").as_number(),
        Some(42.)
    );
    assert_eq!(
        eval("double = (x) ->\n  next = x + 1\n  next * 2\ndouble(20)").as_number(),
        Some(42.)
    );
    assert_eq!(
        eval("n = 0\nwhile n < 3\n  n = n + 1\nn").as_number(),
        Some(3.)
    );
}
#[test]
fn indentation_errors_are_rejected() {
    assert!(Context::new().eval("if true\n  1\n   2").is_err());
    assert!(Context::new().eval("if true\n\t1").is_err());
}
#[test]
fn coffeescript_block_comments_are_ignored_and_require_a_closing_delimiter() {
    assert_eq!(
        eval("### a multiline\n   note with otherwise invalid ` characters\n###\n21 + 21")
            .as_number(),
        Some(42.)
    );
    assert_eq!(eval("### one line ###\n42").as_number(), Some(42.));
    assert!(Context::new().eval("### never closed\n42").is_err());
    assert!(compile("### ignored ###\n42").unwrap().verify().is_ok());
}
#[test]
fn short_circuit_returns_operands() {
    assert_eq!(eval("false and 7").as_bool(), Some(false));
    assert_eq!(eval("true or 7").as_bool(), Some(true));
}
#[test]
fn coffeescript_boolean_and_equality_aliases_preserve_strict_semantics() {
    assert_eq!(eval("yes and on").as_bool(), Some(true));
    assert_eq!(eval("no or off").as_bool(), Some(false));
    assert_eq!(eval("42 is 42").as_bool(), Some(true));
    assert_eq!(eval("42 isnt '42'").as_bool(), Some(true));
    assert_eq!(eval("if yes then 42 else 0").as_number(), Some(42.));
    assert!(Context::new().eval("yes = false").is_err());
    let chunk = compile("left = 1\nright = 1\nleft is right").unwrap();
    assert!(chunk.verify().is_ok());
    assert!(chunk.disassemble().contains("Eq"));
}
#[test]
fn chained_comparisons_evaluate_middle_once_and_short_circuit() {
    assert_eq!(eval("1 < 2 < 3").as_bool(), Some(true));
    assert_eq!(eval("3 < 2 < 1").as_bool(), Some(false));
    assert_eq!(eval("1 is 1 is 1").as_bool(), Some(true));
    let middle_calls = Rc::new(Cell::new(0));
    let last_calls = Rc::new(Cell::new(0));
    let observed_middle = middle_calls.clone();
    let observed_last = last_calls.clone();
    let mut cx = Context::new();
    cx.add_native("middle", move |_| {
        observed_middle.set(observed_middle.get() + 1);
        Ok(Value::Number(2.))
    });
    cx.add_native("last", move |_| {
        observed_last.set(observed_last.get() + 1);
        Ok(Value::Number(3.))
    });
    assert_eq!(
        cx.eval("1 < middle() < last()").unwrap().as_bool(),
        Some(true)
    );
    assert_eq!(middle_calls.get(), 1);
    assert_eq!(last_calls.get(), 1);
    assert_eq!(
        cx.eval("3 < middle() < last()").unwrap().as_bool(),
        Some(false)
    );
    assert_eq!(middle_calls.get(), 2);
    assert_eq!(last_calls.get(), 1);
    let chunk = compile("1 < 2 < 3").unwrap();
    assert!(chunk.verify().is_ok());
    assert!(chunk.disassemble().contains("Rotate3"));
    let bad_swap = Chunk {
        constants: vec![],
        code: vec![Instruction::Swap, Instruction::Return],
    };
    assert!(bad_swap.verify().is_err());
}
#[test]
fn existential_fallback_is_nil_specific_and_short_circuits() {
    assert_eq!(eval("nil ? 42").as_number(), Some(42.));
    assert_eq!(eval("false ? 42").as_bool(), Some(false));
    assert_eq!(eval("0 ? 42").as_number(), Some(0.));
    assert_eq!(eval("nil ? 1 + 2 * 3").as_number(), Some(7.));
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let mut cx = Context::new();
    cx.add_native("fallback", move |_| {
        observed.set(observed.get() + 1);
        Ok(Value::Number(42.))
    });
    assert_eq!(cx.eval("7 ? fallback()").unwrap().as_number(), Some(7.));
    assert_eq!(calls.get(), 0);
    assert_eq!(cx.eval("nil ? fallback()").unwrap().as_number(), Some(42.));
    assert_eq!(calls.get(), 1);
}
#[test]
fn postfix_existence_tests_only_for_nil_and_leave_coalesce_intact() {
    assert_eq!(eval("nil?").as_bool(), Some(false));
    assert_eq!(eval("false?").as_bool(), Some(true));
    assert_eq!(eval("0?").as_bool(), Some(true));
    assert_eq!(eval("value = nil\nvalue? or true").as_bool(), Some(true));
    assert_eq!(eval("nil ? 42").as_number(), Some(42.));
    let chunk = compile("value = nil\nvalue?").unwrap();
    assert!(chunk.verify().is_ok());
    assert!(chunk.disassemble().contains("Exists"));
}
#[test]
fn existential_assignment_binds_only_missing_or_nil_names_and_short_circuits() {
    assert_eq!(
        eval("present = 7\npresent ?= 42\npresent").as_number(),
        Some(7.)
    );
    assert_eq!(
        eval("empty = nil\nempty ?= 42\nempty").as_number(),
        Some(42.)
    );
    assert_eq!(eval("fresh ?= 42\nfresh").as_number(), Some(42.));
    assert_eq!(
        eval("if true then nested ?= 42\nnested").as_number(),
        Some(42.)
    );
    assert_eq!(
        eval("present = 7\npresent ?= missing\npresent").as_number(),
        Some(7.)
    );
    assert!(Context::new().eval("{value: 1}.value ?= 2").is_err());

    let calls = Rc::new(Cell::new(0));
    let counter = calls.clone();
    let mut cx = Context::new();
    cx.add_native("next", move |_| {
        counter.set(counter.get() + 1);
        Ok(Value::Number(42.))
    });
    assert_eq!(
        cx.eval("value = 1\nvalue ?= next()\nvalue")
            .unwrap()
            .as_number(),
        Some(1.)
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(
        cx.eval("value = nil\nvalue ?= next()\nvalue")
            .unwrap()
            .as_number(),
        Some(42.)
    );
    assert_eq!(calls.get(), 1);
    let chunk = compile("fresh ?= 42\nfresh").unwrap();
    assert!(chunk.verify().is_ok());
    assert!(chunk.disassemble().contains("LoadOrNil"));
}
#[test]
fn soak_access_short_circuits_only_nil_receivers() {
    assert!(matches!(eval("record = nil\nrecord?.name"), Value::Nil));
    assert!(matches!(eval("values = nil\nvalues?[0]"), Value::Nil));
    assert_eq!(
        eval("record = {answer: 42}\nrecord?.answer").as_number(),
        Some(42.)
    );
    assert_eq!(eval("values = [40, 2]\nvalues?[1]").as_number(), Some(2.));
    assert_eq!(
        eval("add = (x, y) -> x + y\nadd?([40, 2]...)").as_number(),
        Some(42.)
    );
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let mut cx = Context::new();
    cx.add_native("tick", move |_| {
        observed.set(observed.get() + 1);
        Ok(Value::Number(0.))
    });
    assert!(matches!(
        cx.eval("none = nil\nnone?[tick()]"),
        Ok(Value::Nil)
    ));
    assert!(matches!(
        cx.eval("none = nil\nnone?(tick())"),
        Ok(Value::Nil)
    ));
    assert_eq!(calls.get(), 0);
    assert!(Context::new().eval("record = {}\nrecord?.missing").is_err());
    let chunk = compile("record = nil\nrecord?.name").unwrap();
    assert!(chunk.verify().is_ok());
    assert!(chunk.disassemble().contains("JumpIfNil"));
}
#[test]
fn function_and_lexical_capture() {
    assert_eq!(
        eval("base = 40\nadd = (x) -> x + base\nadd(2)").as_number(),
        Some(42.)
    );
    assert_eq!(
        eval("twice = (x) => x * 2\ntwice(21)").as_number(),
        Some(42.)
    );
    let mut cx = Context::new();
    cx.eval("state = 10\nmake = () -> state = 20\nmake()")
        .unwrap();
    assert_eq!(cx.eval("state").unwrap().as_number(), Some(10.));
    assert_eq!(eval("do -> 42").as_number(), Some(42.));
    assert_eq!(
        eval("sum = (head, tail...) -> head + len(tail)\nsum(40, 1, 2)").as_number(),
        Some(42.)
    );
    assert_eq!(
        eval("count = (items...) -> len(items)\ncount()").as_number(),
        Some(0.)
    );
    assert!(
        Context::new()
            .eval("sum = (head, tail...) -> head\nsum()")
            .is_err()
    );
}
#[test]
fn bare_name_lambda_parameters_support_single_and_multiple_capturing_functions() {
    assert_eq!(
        eval("apply = (function, value) -> function(value)\napply(x -> x + 2, 40)").as_number(),
        Some(42.)
    );
    assert_eq!(
        eval("offset = 2\nadd = left, right -> left + right + offset\nadd(20, 20)").as_number(),
        Some(42.)
    );
    assert!(Context::new().eval("bad = value = 1 -> value").is_err());
    assert!(Context::new().eval("bad = values... -> values").is_err());
    assert!(
        compile("add = left, right -> left + right")
            .unwrap()
            .verify()
            .is_ok()
    );
}
#[test]
fn functions_and_factories_accept_strict_destructuring_parameters() {
    assert_eq!(
        eval("scale = ([left, right], {factor}) -> (left + right) * factor\nscale([20, 1], {factor: 2})")
            .as_number(),
        Some(42.)
    );
    assert_eq!(
        eval("class Point([x, y]) -> {x: x, y: y}\np = Point([20, 22])\np.x + p.y").as_number(),
        Some(42.)
    );
    assert_eq!(
        eval("pick = ({point: [_, value]}) -> value\npick({point: [0, 42]})").as_number(),
        Some(42.)
    );
    assert!(
        Context::new()
            .eval("sum = ([left, right]) -> left + right\nsum([1])")
            .is_err()
    );
    assert!(
        Context::new()
            .eval("sum = ({point: {left, right}}) -> left + right\nsum({point: {left: 1}})")
            .is_err()
    );
    assert!(Context::new().eval("bad = ([x] = [1]) -> x").is_err());
    assert!(Context::new().eval("bad = ([x]...) -> x").is_err());
    let chunk = compile("sum = ([left, right]) -> left + right").unwrap();
    assert!(chunk.verify().is_ok());
    let [Constant::Function { params, .. }] = chunk.constants.as_slice() else {
        panic!("expected one function constant");
    };
    assert!(matches!(params.as_slice(), [Pattern::Array(_)]));
}
#[test]
fn default_parameters_are_evaluated_in_the_callee() {
    assert_eq!(
        eval("add = (x, y = 2) -> x + y\nadd(40)").as_number(),
        Some(42.)
    );
    assert_eq!(
        eval("add = (x, y = 2) -> x + y\nadd(40, nil)").as_number(),
        Some(42.)
    );
    assert_eq!(
        eval("next = (x, y = x + 1) -> y\nnext(41)").as_number(),
        Some(42.)
    );
    assert_eq!(
        eval("base = 40\nvalue = (x = base + 2) -> x\nvalue()").as_number(),
        Some(42.)
    );
    assert_eq!(
        eval("count = (head = 40, tail...) -> head + len(tail)\ncount(41, 1)").as_number(),
        Some(42.)
    );
    let chunk = compile("value = (x, y = 2) -> x + y").unwrap();
    assert!(chunk.verify().is_ok());
    assert!(matches!(
        chunk.constants.as_slice(),
        [Constant::Function { required: 1, .. }]
    ));
    assert!(Context::new().eval("bad = (x = 1, y) -> y").is_err());
    assert!(
        Context::new()
            .eval("one = (x = 1) -> x\none(1, 2)")
            .is_err()
    );
}
#[test]
fn splat_expands_arrays_in_literals_and_calls() {
    assert_eq!(
        eval("tail = [2, 3]\n[1, tail..., 4]").to_string(),
        "[1, 2, 3, 4]"
    );
    assert_eq!(
        eval("sum = (a, b, c) -> a + b + c\nsum(1, [2, 3]...)").as_number(),
        Some(6.)
    );
    assert_eq!(
        eval("prefix = [1]\ntail = [2, 3]\njoin([prefix..., tail...], '-')").as_str(),
        Some("1-2-3")
    );
    assert!(Context::new().eval("[1, 2...]").is_err());
    assert!(Context::new().eval("len(1...)").is_err());
    let chunk = compile("values = [1]\nlen(values...)").unwrap();
    assert!(chunk.disassemble().contains("MergeArrays"));
    assert!(chunk.disassemble().contains("CallSpread"));
}
#[test]
fn recursive_function_uses_explicit_vm_frames() {
    assert_eq!(
        eval("fact = (n) -> if n == 0 then 1 else n * fact(n - 1)\nfact(6)").as_number(),
        Some(720.)
    );
}
#[test]
fn host_function_embedding_is_deliberate_and_small() {
    let mut cx = Context::new();
    cx.add_native("answer", |args| {
        assert!(args.is_empty());
        Ok(Value::Number(42.))
    });
    assert_eq!(cx.eval("answer()").unwrap().as_number(), Some(42.));
    cx.set_global(
        "host_array",
        Value::array(vec![Value::from(40_i64), Value::from(2_i64)]),
    );
    cx.set_global("host_map", Value::map([("answer", Value::from(42_i64))]));
    assert_eq!(
        cx.eval("host_array[0] + host_array[1]")
            .unwrap()
            .as_number(),
        Some(42.)
    );
    assert_eq!(cx.eval("host_map.answer").unwrap().as_number(), Some(42.));
    assert_eq!(
        cx.get_global("host_map").unwrap().as_map().unwrap().len(),
        1
    );
    assert!(cx.get_global("missing").is_none());
    assert_eq!(Value::string("coffee").as_str(), Some("coffee"));
    assert_eq!(
        Value::array(vec![Value::from(1_i64)])
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        Value::map([("x", Value::from(1_i64))])
            .as_map()
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn host_functions_can_return_structured_runtime_errors() {
    let mut cx = Context::new();
    cx.add_native("fail", |_| Err(quickcoffee::Error::runtime("host failure")));
    let error = cx.eval("fail()").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Runtime);
    assert_eq!(error.message(), "host failure");
    assert_eq!(
        cx.eval("try fail() catch error then error")
            .unwrap()
            .as_str(),
        Some("runtime error: host failure")
    );
}
#[test]
fn shared_programs_repeat_without_copying_bytecode() {
    let engine = Engine::new();
    let program = engine.compile_program("1 + 2").unwrap();
    let clone = program.clone();
    assert_eq!(program.disassemble(), clone.disassemble());
    assert!(program.verify().is_ok());
    let mut cx = Context::new();
    assert_eq!(cx.run_program(&program).unwrap().as_number(), Some(3.));
    assert_eq!(cx.run_program(&clone).unwrap().as_number(), Some(3.));
    let top_level = quickcoffee::compile_program("2 + 2").unwrap();
    assert_eq!(cx.run_program(&top_level).unwrap().as_number(), Some(4.));
}
#[test]
fn while_and_fuel() {
    assert_eq!(
        eval("n = 0\nwhile n < 3 then n = n + 1\nn").as_number(),
        Some(3.)
    );
    assert!(
        Context::new()
            .with_fuel(20)
            .eval("while true then 1")
            .is_err()
    );
    assert_eq!(
        eval("n = 0\nuntil n == 3 then n = n + 1\nn").as_number(),
        Some(3.)
    );
}
#[test]
fn postfix_while_and_until_repeat_assignments_and_strict_destructuring() {
    assert_eq!(
        eval("n = 0\nn = n + 1 while n < 3\nn").as_number(),
        Some(3.)
    );
    assert_eq!(
        eval("n = 0\nn = n + 1 until n == 3\nn").as_number(),
        Some(3.)
    );
    assert_eq!(
        eval("a = 0\nb = 0\n[a, b] = [a + 1, b + 1] while a < 2\na + b").as_number(),
        Some(4.)
    );
    assert!(
        compile("n = 0\nn = n + 1 while n < 3\nn")
            .unwrap()
            .verify()
            .is_ok()
    );
    assert!(compile("break while true").unwrap().verify().is_ok());
    assert!(matches!(eval("break while true"), Value::Nil));
}
#[test]
fn loop_is_an_infinite_while_form_with_normal_break_continue_and_fuel_rules() {
    assert_eq!(
        eval("n = 0\nloop\n  n = n + 1\n  break if n == 3\nn").as_number(),
        Some(3.)
    );
    assert!(Context::new().with_fuel(10).eval("loop 1").is_err());
    assert!(compile("loop break").unwrap().verify().is_ok());
}
#[test]
fn for_loops_break_and_continue_are_bytecode_control_flow() {
    assert_eq!(
        eval("sum = 0\nfor n in range(1, 8) then if n == 5 then break else sum = sum + n\nsum")
            .as_number(),
        Some(10.)
    );
    assert_eq!(
        eval("sum = 0\nfor n in [1, 2, 3, 4] then if n == 2 then continue else sum = sum + n\nsum")
            .as_number(),
        Some(8.)
    );
    assert_eq!(
        eval("sum = 0\nfor outer in [1, 2] then for inner in [1, 2, 3] then if inner == 2 then break else sum = sum + outer * inner\nsum")
            .as_number(),
        Some(3.)
    );
    assert_eq!(
        eval("sum = 0\nfor own key, value of {a: 20, b: 22} then sum = sum + value\nsum")
            .as_number(),
        Some(42.)
    );
    assert_eq!(
        eval("for value, index in [10, 20, 30] then value + index").to_string(),
        "[10, 21, 32]"
    );
    assert_eq!(
        eval("for value, index in [10, 20, 30] by 2 then index").to_string(),
        "[0, 2]"
    );
    assert_eq!(
        eval("for value, index in [10..14] when value % 2 == 0 then index").to_string(),
        "[0, 2, 4]"
    );
    assert!(
        compile("for value, index in [1..3] then value + index")
            .unwrap()
            .verify()
            .is_ok()
    );
}
#[test]
fn string_iteration_uses_unicode_scalars_and_optional_scalar_indices() {
    assert_eq!(
        eval("for character in 'a☕中' then character").to_string(),
        "[a, ☕, 中]"
    );
    assert_eq!(
        eval("for character, index in 'a☕中' then index").to_string(),
        "[0, 1, 2]"
    );
    assert_eq!(
        eval("text = 'ab'\nfor character in text then character").to_string(),
        "[a, b]"
    );
    assert_eq!(
        eval("for character in 'a☕中' when character == '☕' then character").to_string(),
        "[☕]"
    );
    assert!(
        Context::new()
            .eval("for character in 'abc' by 2 then character")
            .is_err()
    );
    let chunk = compile("for character in 'abc' then character").unwrap();
    assert!(chunk.verify().is_ok());
    assert!(chunk.disassemble().contains("IterStartEnumerable"));
}
#[test]
fn for_loop_bindings_support_strict_recursive_patterns_atomically() {
    assert_eq!(
        eval("for [left, right] in [[1, 2], [3, 4]] then left + right").to_string(),
        "[3, 7]"
    );
    assert_eq!(
        eval("for {point: {x, y}} in [{point: {x: 20, y: 22}}] then x + y").to_string(),
        "[42]"
    );
    assert_eq!(
        eval("for _, value of {a: 20, b: 22} then value").to_string(),
        "[20, 22]"
    );
    assert_eq!(
        eval("left = 10\ntry for [left, right] in [[1]] then left catch error then left")
            .as_number(),
        Some(10.)
    );
    assert!(compile("for first, second, third in [[1, 2]] then first").is_err());
    let chunk = compile("for [left, right] in [[20, 22]] then left + right").unwrap();
    assert!(chunk.verify().is_ok());
    assert!(chunk.disassemble().contains("IterNext"));
}
#[test]
fn for_expressions_collect_body_values_and_omit_filtered_or_broken_items() {
    assert_eq!(eval("for n in [1..3] then n * 2").to_string(), "[2, 4, 6]");
    assert_eq!(
        eval("for n in [1..5] when n > 2 then n * 2").to_string(),
        "[6, 8, 10]"
    );
    assert_eq!(
        eval("for n in [1..5] then if n == 3 then break else n").to_string(),
        "[1, 2]"
    );
    let chunk = compile("for n in [1..3] then n * 2").unwrap();
    assert!(chunk.verify().is_ok());
    assert!(chunk.disassemble().contains("Append"));
    let discarded = compile("for n in [1..3] then n * 2\n0").unwrap();
    assert!(discarded.verify().is_ok());
    assert!(!discarded.disassemble().contains("Append"));
    let discarded_in_block = compile("run = ->\n  for n in [1..3] then n * 2\n  0\nrun()").unwrap();
    assert!(discarded_in_block.verify().is_ok());
    assert!(!discarded_in_block.disassemble().contains("Append"));
}
#[test]
fn postfix_for_comprehensions_reuse_strict_iteration_and_collection_rules() {
    assert_eq!(eval("n * 2 for n in [1..3]").to_string(), "[2, 4, 6]");
    assert_eq!(eval("[n * 2 for n in [1..3]]").to_string(), "[2, 4, 6]");
    assert_eq!(
        eval("value + index for value, index in [10, 20, 30] by 2").to_string(),
        "[10, 32]"
    );
    assert_eq!(
        eval("value for value in [1..5] when value % 2 == 0").to_string(),
        "[2, 4]"
    );
    assert_eq!(
        eval("value * 2 for own key, value of {a: 1, b: 2}").to_string(),
        "[2, 4]"
    );
    assert_eq!(
        eval("right for [left, right] in [[1, 2], [3, 4]]").to_string(),
        "[2, 4]"
    );
    let chunk = compile("[n + 1 for n in [1..3]]").unwrap();
    assert!(chunk.verify().is_ok());
    assert!(chunk.disassemble().contains("Append"));
    assert!(Context::new().eval("[n for n in [1], 2]").is_err());
}
#[test]
fn array_for_by_uses_a_strict_once_evaluated_positive_integer_step() {
    assert_eq!(
        eval("sum = 0\nfor n in [1..9] by 3 then sum = sum + n\nsum").as_number(),
        Some(12.)
    );
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let mut cx = Context::new();
    cx.add_native("step", move |_| {
        observed.set(observed.get() + 1);
        Ok(Value::Number(2.))
    });
    assert_eq!(
        cx.eval("sum = 0\nfor n in [1..5] by step() then sum = sum + n\nsum")
            .unwrap()
            .as_number(),
        Some(9.)
    );
    assert_eq!(calls.get(), 1);
    for source in [
        "for n in [1, 2] by 0 then n",
        "for n in [1, 2] by 1.5 then n",
        "for n in [1, 2] by true then n",
        "for own key, value of {a: 1} by 2 then value",
    ] {
        assert!(Context::new().eval(source).is_err(), "{source}");
    }
    assert!(
        compile("for n in [1..5] by 2 then n")
            .unwrap()
            .verify()
            .is_ok()
    );
}
#[test]
fn loop_control_outside_a_loop_is_rejected() {
    assert!(Context::new().eval("break").is_err());
    assert!(Context::new().eval("continue").is_err());
    assert!(Context::new().eval("for x in 3 then x").is_err());
    assert!(Context::new().eval("for x of {a: 1} then x").is_err());
    assert!(Context::new().eval("for x, y, z in [1] then x").is_err());
    assert_eq!(
        eval("sum = 0\nfor n in [1..5] when n % 2 == 1 then sum = sum + n\nsum").as_number(),
        Some(9.)
    );
    assert_eq!(
        eval("count = 0\nfor own key, value of {a: 1, b: 2} when value > 1 then count = count + 1\ncount").as_number(),
        Some(1.)
    );
    let filtered = compile("for n in [1..3] when n > 1 then n").unwrap();
    filtered.verify().unwrap();
    assert!(Context::new().eval("for n in [1] when 1 then n").is_err());
}
#[test]
fn maps_ranges_and_indexing() {
    assert_eq!(eval("{name: 'coffee'}['name']").as_str(), Some("coffee"));
    assert_eq!(eval("{name: 'coffee'}.name").as_str(), Some("coffee"));
    assert_eq!(
        eval("name = 'coffee'\n{name}.name").as_str(),
        Some("coffee")
    );
    assert_eq!(
        eval("name = 'coffee'\nanswer = 42\n{name, answer}.answer").as_number(),
        Some(42.)
    );
    assert!(Context::new().eval("{'name'}").is_err());
    assert_eq!(eval("range(2, 5)[1]").as_number(), Some(3.));
    assert_eq!(eval("[2..4]").to_string(), "[2, 3, 4]");
    assert_eq!(eval("[2...4]").to_string(), "[2, 3]");
    assert!(Context::new().eval("[1.5..3]").is_err());
    assert!(Context::new().eval("[0...1000001]").is_err());
    let mut cx = Context::new();
    cx.add_native("range", |_| {
        Ok(Value::Array(Rc::new(vec![Value::Number(99.)])))
    });
    assert_eq!(cx.eval("[2..4]").unwrap().to_string(), "[2, 3, 4]");
}
#[test]
fn array_slices_use_strict_once_evaluated_bounds_and_nil_safe_suffixes() {
    assert_eq!(eval("[0..4][1..3]").to_string(), "[1, 2, 3]");
    assert_eq!(eval("[0..4][1...3]").to_string(), "[1, 2]");
    assert_eq!(eval("[0..4][-3..-1]").to_string(), "[2, 3, 4]");
    assert!(matches!(eval("none = nil\nnone?[missing...1]"), Value::Nil));
    assert!(Context::new().eval("[1, 2][0..2]").is_err());
    assert!(Context::new().eval("[1, 2][0.5...1]").is_err());
    assert!(Context::new().eval("{items: [1]}[0...1]").is_err());

    let calls = Rc::new(Cell::new(0));
    let counter = calls.clone();
    let mut cx = Context::new();
    cx.add_native("bound", move |_| {
        let call = counter.get() + 1;
        counter.set(call);
        Ok(Value::Number(if call == 1 { 1. } else { 3. }))
    });
    assert_eq!(
        cx.eval("[0..4][bound()...bound()]").unwrap().to_string(),
        "[1, 2]"
    );
    assert_eq!(calls.get(), 2);
    let chunk = compile("[0..4][1..3]").unwrap();
    assert!(chunk.verify().is_ok());
    assert!(chunk.disassemble().contains("Slice(true)"));
}
#[test]
fn membership_uses_arrays_and_prototype_free_map_keys() {
    assert_eq!(eval("2 in [1, 2, 3]").as_bool(), Some(true));
    assert_eq!(eval("4 in [1, 2, 3]").as_bool(), Some(false));
    assert_eq!(eval("'name' of {name: 'coffee'}").as_bool(), Some(true));
    assert_eq!(eval("'missing' of {name: 'coffee'}").as_bool(), Some(false));
    assert!(Context::new().eval("2 in {name: 2}").is_err());
    assert!(Context::new().eval("2 of {name: 2}").is_err());
}
#[test]
fn negated_membership_uses_the_same_strict_array_and_map_rules() {
    assert_eq!(eval("3 not in [1, 2]").as_bool(), Some(true));
    assert_eq!(eval("2 not in [1, 2]").as_bool(), Some(false));
    assert_eq!(eval("'missing' not of {present: 1}").as_bool(), Some(true));
    assert_eq!(eval("'present' not of {present: 1}").as_bool(), Some(false));
    assert!(Context::new().eval("1 not in 2").is_err());
    assert!(Context::new().eval("'x' not of [1]").is_err());
    let chunk = compile("value = 3\nitems = [1, 2]\nvalue not in items").unwrap();
    assert!(chunk.verify().is_ok());
    assert!(chunk.disassemble().contains("Contains"));
    assert!(chunk.disassemble().contains("Not"));
}
#[test]
fn classes_are_prototype_free_map_factories() {
    assert_eq!(
        eval("class Point(x, y) -> {x: x, y: y}\np = Point(3, 4)\np.x + p.y").as_number(),
        Some(7.)
    );
    assert_eq!(
        eval("class Empty() -> {}\ntype(Empty)").as_str(),
        Some("function")
    );
    assert_eq!(
        eval("class Point(x, y = 2) -> {x: x, y: y}\nPoint(40).x + Point(40).y").as_number(),
        Some(42.)
    );
    assert!(
        Context::new()
            .eval("class Point(x, y) -> {x: x, y: y}\nPoint(1)")
            .is_err()
    );
    assert!(Context::new().eval("class Bad(x = 1, y) -> x").is_err());
}
#[test]
fn double_quoted_strings_interpolate_quickcoffee_expressions() {
    assert_eq!(
        eval("name = 'Coffee'\n\"Hello #{name}, #{2 + 3}!\"").as_str(),
        Some("Hello Coffee, 5!")
    );
    assert_eq!(eval("'#{1 + 1}'").as_str(), Some("#{1 + 1}"));
    assert_eq!(eval("\"#{ {key: '}'} .key }\"").as_str(), Some("}"));
    assert!(Context::new().eval("\"#{unknown}\"").is_err());
}
#[test]
fn triple_quoted_heredocs_keep_newlines_and_double_quote_interpolation() {
    assert_eq!(
        eval("name = 'Coffee'\nmessage = \"\"\"Hello #{name}\nnext\"\"\"\nmessage").as_str(),
        Some("Hello Coffee\nnext")
    );
    assert_eq!(
        eval("'''#{1 + 1}\nnext''' ").as_str(),
        Some("#{1 + 1}\nnext")
    );
    assert!(Context::new().eval("\"\"\"unfinished").is_err());
    assert!(compile("\"\"\"line\nend\"\"\"").unwrap().verify().is_ok());
}
#[test]
fn redesigned_standard_library_is_function_based_not_prototype_based() {
    assert_eq!(eval("join(keys({b: 2, a: 1}), ',')").as_str(), Some("a,b"));
    assert_eq!(eval("split('a,b', ',')[1]").as_str(), Some("b"));
    assert_eq!(eval("str(values({a: 42})[0])").as_str(), Some("42"));
    assert!(Context::new().eval("assert(false, 'expected')").is_err());
}
#[test]
fn array_destructuring_is_strict_and_has_an_explicit_ignore_name() {
    assert_eq!(
        eval("left, right = [20, 22]\nleft + right").as_number(),
        Some(42.)
    );
    assert_eq!(eval("_, answer = [0, 42]\nanswer").as_number(), Some(42.));
    assert!(Context::new().eval("one, two = [1]").is_err());
    assert!(Context::new().eval("one, two = 1").is_err());
    let mut cx = Context::new();
    cx.eval("first = 99").unwrap();
    assert!(cx.eval("first, second = [1]").is_err());
    assert_eq!(cx.eval("first").unwrap().as_number(), Some(99.));
    assert!(cx.eval("second").is_err());
}
#[test]
fn map_destructuring_supports_renaming_and_is_atomic() {
    assert_eq!(
        eval("{name, count: total} = {name: 'coffee', count: 42}\nname").as_str(),
        Some("coffee")
    );
    assert_eq!(
        eval("{name, count: total} = {name: 'coffee', count: 42}\ntotal").as_number(),
        Some(42.)
    );
    let mut cx = Context::new();
    cx.eval("stable = 9").unwrap();
    assert!(cx.eval("{stable, absent} = {stable: 1}").is_err());
    assert_eq!(cx.eval("stable").unwrap().as_number(), Some(9.));
    assert!(cx.eval("absent").is_err());
}
#[test]
fn nested_destructuring_is_strict_and_atomic() {
    assert_eq!(
        eval("[first, [middle, last]] = [1, [2, 39]]\nfirst + middle + last").as_number(),
        Some(42.)
    );
    assert_eq!(
        eval("{point: {x, y}, labels: [_, name]} = {point: {x: 20, y: 22}, labels: ['skip', 'coffee']}\nx + y")
            .as_number(),
        Some(42.)
    );
    assert_eq!(
        eval("{point: {x, y}, labels: [_, name]} = {point: {x: 20, y: 22}, labels: ['skip', 'coffee']}\nname")
            .as_str(),
        Some("coffee")
    );
    let mut cx = Context::new();
    cx.eval("first = 40\nsecond = 2").unwrap();
    assert!(cx.eval("[first, [second, missing]] = [1, [2]]").is_err());
    assert_eq!(cx.eval("first + second").unwrap().as_number(), Some(42.));
    assert!(
        cx.eval("{point: {first, missing}} = {point: {first: 0}}")
            .is_err()
    );
    assert_eq!(cx.eval("first").unwrap().as_number(), Some(40.));
    let chunk = compile("[a, {point: [b, c]}] = [1, {point: [2, 3]}]").unwrap();
    assert!(chunk.verify().is_ok());
    assert!(chunk.disassemble().contains("Destructure"));
    let bad_destructure = Chunk {
        constants: vec![],
        code: vec![
            Instruction::Destructure(Pattern::Array(vec![])),
            Instruction::Return,
        ],
    };
    assert!(bad_destructure.verify().is_err());
    let bad_rest_pattern = Chunk {
        constants: vec![Constant::Value(Value::array(vec![]))],
        code: vec![
            Instruction::Constant(0),
            Instruction::Destructure(Pattern::Rest("tail".into())),
            Instruction::Return,
        ],
    };
    assert!(bad_rest_pattern.verify().is_err());
    let bad_rest_position = Chunk {
        constants: vec![Constant::Value(Value::array(vec![]))],
        code: vec![
            Instruction::Constant(0),
            Instruction::Destructure(Pattern::Array(vec![
                Pattern::Rest("tail".into()),
                Pattern::Bind("next".into()),
            ])),
            Instruction::Return,
        ],
    };
    assert!(bad_rest_position.verify().is_err());
}
#[test]
fn array_destructuring_rest_binds_an_immutable_tail() {
    assert_eq!(
        eval("[head, tail...] = [1, 2, 3]\nlen(tail) + head").as_number(),
        Some(3.)
    );
    assert_eq!(eval("[head, tail...] = [1]\ntail").to_string(), "[]");
    assert_eq!(
        eval("collect = ([head, tail...]) -> [head, tail]\ncollect([1, 2, 3])").to_string(),
        "[1, [2, 3]]"
    );
    assert_eq!(
        eval("for [head, tail...] in [[1, 2, 3], [4]] then len(tail) + head").to_string(),
        "[3, 4]"
    );
    assert!(Context::new().eval("[head, tail...] = []").is_err());
    assert!(Context::new().eval("[head..., tail] = [1, 2]").is_err());
    assert!(Context::new().eval("[head..., tail...] = [1, 2]").is_err());
    let chunk = compile("[head, tail...] = [1, 2, 3]").unwrap();
    assert!(chunk.verify().is_ok());
}
#[test]
fn rejects_js_and_unknown_names() {
    assert!(Context::new().eval("`alert(1)`").is_err());
    assert!(Context::new().eval("undefined").is_err());
}
#[test]
fn bytecode_is_verified_and_ends_in_return() {
    let chunk = compile("1 + 2").unwrap();
    chunk.verify().unwrap();
    assert!(chunk.disassemble().contains("Return"));
}
#[test]
fn verifier_rejects_untrusted_bad_bytecode() {
    let empty = Chunk {
        constants: vec![],
        code: vec![],
    };
    assert!(empty.verify().is_err());
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| empty.verify())).is_ok());
    let bad_constant = Chunk {
        constants: vec![],
        code: vec![Instruction::Constant(9), Instruction::Return],
    };
    assert!(bad_constant.verify().is_err());
    assert!(Context::new().run(bad_constant).is_err());
    let bad_program = Program::from(Chunk {
        constants: vec![],
        code: vec![Instruction::Pop, Instruction::Return],
    });
    assert!(Context::new().run_program(&bad_program).is_err());
    let bad_jump = Chunk {
        constants: vec![],
        code: vec![Instruction::Jump(9), Instruction::Return],
    };
    assert!(bad_jump.verify().is_err());
    let bad_nil_jump = Chunk {
        constants: vec![],
        code: vec![Instruction::JumpIfNil(0), Instruction::Return],
    };
    assert!(bad_nil_jump.verify().is_err());
    let bad_stack = Chunk {
        constants: vec![],
        code: vec![Instruction::Pop, Instruction::Return],
    };
    assert!(bad_stack.verify().is_err());
    let bad_dup = Chunk {
        constants: vec![],
        code: vec![Instruction::Dup, Instruction::Return],
    };
    assert!(bad_dup.verify().is_err());
    let bad_splat_merge = Chunk {
        constants: vec![],
        code: vec![Instruction::MergeArrays(1), Instruction::Return],
    };
    assert!(bad_splat_merge.verify().is_err());
    let bad_splat_call = Chunk {
        constants: vec![],
        code: vec![Instruction::CallSpread, Instruction::Return],
    };
    assert!(bad_splat_call.verify().is_err());
    let bad_handler = Chunk {
        constants: vec![Constant::Value(Value::Nil)],
        code: vec![
            Instruction::Constant(0),
            Instruction::EndTry,
            Instruction::Return,
        ],
    };
    assert!(bad_handler.verify().is_err());
    let bad_iterator = Chunk {
        constants: vec![],
        code: vec![
            Instruction::IterNext {
                patterns: vec![Pattern::Bind("x".into())],
                end: 0,
            },
            Instruction::Return,
        ],
    };
    assert!(bad_iterator.verify().is_err());
    let invalid_inner = Chunk {
        constants: vec![],
        code: vec![Instruction::Pop, Instruction::Return],
    };
    let bad_nested_function = Chunk {
        constants: vec![Constant::Function {
            params: vec![],
            required: 0,
            rest: None,
            chunk: Rc::new(invalid_inner),
        }],
        code: vec![Instruction::MakeFunction(0), Instruction::Return],
    };
    assert!(bad_nested_function.verify().is_err());

    let mut deeply_nested = Pattern::Ignore;
    for _ in 0..300 {
        deeply_nested = Pattern::Array(vec![deeply_nested]);
    }
    let bad_deep_pattern = Chunk {
        constants: vec![],
        code: vec![Instruction::Destructure(deeply_nested), Instruction::Return],
    };
    assert!(bad_deep_pattern.verify().is_err());

    let bad_ignored_rest = Chunk {
        constants: vec![],
        code: vec![
            Instruction::Destructure(Pattern::Array(vec![Pattern::Rest("_".into())])),
            Instruction::Return,
        ],
    };
    assert!(bad_ignored_rest.verify().is_err());
}

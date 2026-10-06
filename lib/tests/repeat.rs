use pear::combinators::repeat;
use pear::input::{Pear, Result, Rewind, Text};
use pear::macros::{parse, parse_declare, parser};
use pear::parsers::*;

parse_declare!(Input<'a>(Token = char, Slice = &'a str, Many = &'a str));

/// Repeats `none`, a parser that always succeeds without consuming input.
#[parser]
fn repeat_none<'a, I>(input: &mut Pear<I>) -> Result<Vec<&'a str>, I>
    where I: Input<'a> + Rewind,
          I::Marker: PartialEq,
{
    Ok(repeat(none)?)
}

/// Repeats `eat_any`, a parser that consumes exactly one token per success.
#[parser]
fn repeat_any<'a, I>(input: &mut Pear<I>) -> Result<Vec<char>, I>
    where I: Input<'a> + Rewind,
          I::Marker: PartialEq,
{
    Ok(repeat(eat_any)?)
}

/// Never matches the input `"abc"`; `repeat_no_match` thus always succeeds
/// with an empty collection without consuming input.
#[parser]
fn eat_x<'a, I: Input<'a>>(input: &mut Pear<I>) -> Result<char, I> {
    eat('x')?
}

#[parser]
fn repeat_no_match<'a, I>(input: &mut Pear<I>) -> Result<Vec<char>, I>
    where I: Input<'a> + Rewind,
          I::Marker: PartialEq,
{
    Ok(repeat(eat_x)?)
}

/// Outer `repeat` whose item is an inner `repeat` that succeeds empty.
#[parser]
fn nested_repeat_empty<'a, I>(input: &mut Pear<I>) -> Result<Vec<Vec<char>>, I>
    where I: Input<'a> + Rewind,
          I::Marker: PartialEq,
{
    Ok(repeat(repeat_no_match)?)
}

/// Outer `repeat` whose item is an inner `repeat` that itself stalls.
#[parser]
fn nested_repeat_none<'a, I>(input: &mut Pear<I>) -> Result<Vec<Vec<&'a str>>, I>
    where I: Input<'a> + Rewind,
          I::Marker: PartialEq,
{
    Ok(repeat(repeat_none)?)
}

#[test]
fn empty_success_errors_in_finite_steps() {
    let result = parse!(repeat_none: Text::from("abc"));
    let err = result.expect_err("zero-progress repeat must fail");
    assert!(format!("{}", err).contains("without consuming input"));
}

#[test]
fn zero_progress_error_points_at_stall() {
    let mut input: Pear<Text> = Pear::new(Text::from("abc"));
    let mut consumed = false;
    let result: Result<Vec<&str>, Text> = repeat(&mut input, |i| {
        if !consumed {
            consumed = true;
            eat_any(i)?;
        }

        none(i)
    });

    let err = result.expect_err("repeat must fail on zero progress");
    // The first iteration consumed one token; the stall is reported there.
    assert_eq!(err.info.context.start.2, 1);
    assert!(format!("{}", err).contains("without consuming input"));
}

#[test]
fn three_chars_still_succeed_three_times() {
    let result = parse!(repeat_any: Text::from("abc"));
    assert_eq!(result.unwrap(), vec!['a', 'b', 'c']);
}

#[test]
fn nested_repeat_detects_inner_empty_success() {
    // The inner repeat succeeds with an empty collection without consuming
    // input, so the outer repeat must report zero progress.
    let result = parse!(nested_repeat_empty: Text::from("abc"));
    let err = result.expect_err("nested zero-progress repeat must fail");
    assert!(format!("{}", err).contains("without consuming input"));
}

#[test]
fn nested_repeat_detects_inner_stall() {
    // The inner repeat stalls on `none`; its error surfaces (as a failure of
    // the outer item) instead of looping forever.
    let result = parse!(nested_repeat_none: Text::from("abc"));
    assert!(result.is_err());
}

use pear::input::Pear;
use pear::error::Expected;
use pear::{macros::*, parsers::*, combinators::repeat};

type Input<'a> = Pear<pear::input::Text<'a>>;
type Result<'a, T> = pear::input::Result<T, Input<'a>>;

/// A parser that always succeeds without consuming any input.
#[parser]
fn empty_success<'a>(input: &mut Input<'a>) -> Result<'a, ()> {
    let _ = &input;
}

/// A parser that consumes exactly one token per success.
#[parser]
fn one_char<'a>(input: &mut Input<'a>) -> Result<'a, char> {
    eat_any()?
}

/// Repeats a parser that always succeeds without consuming input.
#[parser]
fn repeat_empty<'a>(input: &mut Input<'a>) -> Result<'a, Vec<()>> {
    let collected: Vec<()> = repeat(empty_success)?;
    collected
}

/// Repeats a parser that consumes exactly one token per success.
#[parser]
fn repeat_any<'a>(input: &mut Input<'a>) -> Result<'a, Vec<char>> {
    let collected: Vec<char> = repeat(one_char)?;
    collected
}

/// A `repeat` nested inside another `repeat`: the inner repetition is over an
/// empty-success parser, so the guard must fire through the outer `repeat`.
#[parser]
fn repeat_nested_empty<'a>(input: &mut Input<'a>) -> Result<'a, Vec<Vec<()>>> {
    let collected: Vec<Vec<()>> = repeat(repeat_empty)?;
    collected
}

/// Consumes two tokens, then repeats an empty-success parser.
#[parser]
fn two_then_empty<'a>(input: &mut Input<'a>) -> Result<'a, (char, char, Vec<()>)> {
    let first = one_char()?;
    let second = one_char()?;
    let rest: Vec<()> = repeat(empty_success)?;
    (first, second, rest)
}

fn assert_zero_progress<T>(result: pear::input::Result<T, Input<'_>>) {
    match result {
        Err(error) => match &error.error {
            Expected::Other(message) => {
                assert!(message.contains("without consuming input"),
                    "unexpected message: {}", message);
            }
            other => panic!("expected zero-progress error, got: {:?}", other),
        },
        Ok(_) => panic!("expected zero-progress error, but parse succeeded"),
    }
}

#[test]
fn empty_success_errors_in_finite_steps() {
    // An empty-success parser repeated must not loop forever: it errors out.
    assert_zero_progress(parse!(repeat_empty: Input::new("abc")));
    assert_zero_progress(parse!(repeat_empty: Input::new("")));
}

#[test]
fn zero_progress_error_points_at_repeat_start() {
    // Consume two tokens, then hit the zero-progress guard. The error's
    // context must begin where the repetition began: offset 2, line 1 col 3.
    let result = parse!(two_then_empty: Input::new("abc"));
    let error = result.expect_err("expected zero-progress error");
    assert_eq!(error.info.context.start, (1, 3, 2));
}

#[test]
fn consuming_three_chars_succeeds_three_times() {
    let result = parse!(repeat_any: Input::new("abc"));
    assert_eq!(result.unwrap(), vec!['a', 'b', 'c']);
}

#[test]
fn empty_input_yields_empty_collection() {
    let result = parse!(repeat_any: Input::new(""));
    assert_eq!(result.unwrap(), Vec::<char>::new());
}

#[test]
fn nested_repeat_detects_zero_progress() {
    assert_zero_progress(parse!(repeat_nested_empty: Input::new("abc")));
    assert_zero_progress(parse!(repeat_nested_empty: Input::new("")));
}

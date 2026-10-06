//! Targeted tests for the `consumed` position-reporting combinator.
//!
//! The fixtures here are intentionally tiny: they assert that a parser is run
//! exactly once and that the reported start/end cursors are the real consumed
//! positions, including after a committed failure, without landing in the
//! middle of a multibyte UTF-8 character.

use std::cell::Cell;

use pear::combinators::{collect, consumed};
use pear::input::{Input, Pear, Text};
use pear::{macros::*, parsers::*};

type PInput<'a> = Pear<Text<'a>>;
type PResult<'a, T> = pear::input::Result<T, PInput<'a>>;

/// Eats one multibyte character and then commits to a failure at EOF.
#[parser]
fn eat_then_eof<'a>(input: &mut PInput<'a>, c: char) -> PResult<'a, ()> {
    eat(c)?;
    eof()?;
}

#[test]
fn committed_failure_reports_true_end_after_multibyte() {
    // `'中'` is three UTF-8 bytes; the cursor must end at byte 3, never at
    // byte 1 or 2 (inside the character).
    let source = "中x";
    let mut input = PInput::from(Text::from(source));
    let report = consumed(&mut input, |i| eat_then_eof(i, '中'));

    assert!(report.result.is_err());
    assert_eq!(report.start, 0);
    assert_eq!(report.end, '中'.len_utf8());
    assert_eq!(report.context.snippet, Some("中"));
    assert!(report.is_committed_failure());

    // The end cursor is a real UTF-8 boundary: slicing the original input
    // there yields a valid prefix, while a mid-character offset would panic.
    assert_eq!(&source[..report.end], "中");
    assert_eq!(source.get(report.end..), Some("x"));
}

#[test]
fn committed_failure_end_is_char_boundary_for_emoji() {
    // A four-byte multibyte character followed by trailing input.
    let source = "🦀ab";
    let mut input = PInput::from(Text::from(source));
    let report = consumed(&mut input, |i| eat_then_eof(i, '🦀'));

    assert!(report.result.is_err());
    assert_eq!(report.start, 0);
    assert_eq!(report.end, '🦀'.len_utf8());
    assert_eq!(report.context.snippet, Some("🦀"));
    assert_eq!(&source[..report.end], "🦀");
    // A mid-character index would make these slicing operations fail.
    assert_eq!(source.get(..report.end), Some("🦀"));
    assert_eq!(source.get(report.end..), Some("ab"));
}

#[test]
fn failed_input_is_not_reported_as_unconsumed() {
    // After the report, the input must still be positioned past the consumed
    // character: `consumed` neither rewinds nor retries.
    let source = "é!";
    let mut input = PInput::from(Text::from(source));
    let report = consumed(&mut input, |i| eat_then_eof(i, 'é'));

    assert!(report.result.is_err());
    assert_eq!(report.end, 'é'.len_utf8());
    assert_eq!(input.token(), Some('!'));
}

#[test]
fn success_runs_parser_once_and_reports_extent() {
    let calls = Cell::new(0u32);
    let mut input = PInput::from(Text::from("abc"));

    let report = consumed(&mut input, |i| {
        calls.set(calls.get() + 1);
        eat_slice(i, "abc")
    });

    assert_eq!(calls.get(), 1, "parser must run exactly once");
    assert_eq!(report.start, 0);
    assert_eq!(report.end, 3);
    assert_eq!(report.context.snippet, Some("abc"));
    assert!(report.is_ok());
    assert!(!report.is_committed_failure());
    assert_eq!(report.result.unwrap(), "abc");
}

#[test]
fn committed_failure_runs_parser_once() {
    let calls = Cell::new(0u32);
    let mut input = PInput::from(Text::from("中x"));

    let report = consumed(&mut input, |i| {
        calls.set(calls.get() + 1);
        eat_then_eof(i, '中')
    });

    assert_eq!(calls.get(), 1, "parser must run exactly once even on failure");
    assert!(report.result.is_err());
    assert_eq!(report.end, '中'.len_utf8());
}

//
// Two combinator composition examples.
//

/// Composition example 1: `consumed` around `take_while` on a multibyte
/// alphabet; the report bounds only the accepted run.
#[test]
fn composed_with_take_while() {
    let mut input = PInput::from(Text::from("αβγ.."));
    let report = consumed(&mut input, |i| {
        take_while(i, |c| c.is_alphabetic())
    });

    assert!(report.is_ok());
    assert_eq!(report.start, 0);
    assert_eq!(report.end, 6);
    assert_eq!(report.context.snippet, Some("αβγ"));
    assert_eq!(report.result.unwrap(), "αβγ");
    // Only the letters were consumed; the dots remain.
    assert_eq!(input.token(), Some('.'));
}

/// Composition example 2: `consumed` around the `collect` combinator. Each
/// item eats one multibyte letter; the whole run is reported as one extent.
#[parser]
fn letter<'a>(input: &mut PInput<'a>) -> PResult<'a, char> {
    eat_if(input, |c: &char| c.is_alphabetic())
}

#[test]
fn composed_with_collect() {
    let mut input = PInput::from(Text::from("αβγ"));
    let report = consumed(&mut input, |i| {
        collect::<Vec<char>, _, _, _>(i, letter)
    });

    assert!(report.is_ok());
    assert_eq!(report.start, 0);
    assert_eq!(report.end, "αβγ".len());
    assert_eq!(report.context.snippet, Some("αβγ"));
    assert_eq!(report.result.unwrap(), vec!['α', 'β', 'γ']);
}

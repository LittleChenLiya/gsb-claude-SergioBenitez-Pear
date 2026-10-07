//! Acceptance tests for the opt-in `recovery` feature: the explicit
//! "recover to a delimiter" combinator.
//!
//! The whole file is gated on the feature: when the feature is not enabled
//! this test file is empty, and the default interface behaves exactly as
//! before.
#![cfg(feature = "recovery")]

use pear::input::{Cursor, Pear, Text};
use pear::macros::parser;
use pear::parsers::*;
use pear::recovery::{recover_until, Recovery, RecoveryPoint};

type CResult<'a, O> = pear::input::Result<O, Cursor<&'a str>>;
type TResult<'a, O> = pear::input::Result<O, Text<'a>>;

/// A parser that consumes `a` and then requires `b`; fails on the second token.
#[parser]
fn a_then_b<'a>(input: &mut Pear<Cursor<&'a str>>) -> CResult<'a, char> {
    let a = eat('a')?;
    let _b = eat('b')?;
    Ok(a)
}

/// A parser that consumes `a` and then requires `b` over `Text`.
#[parser]
fn a_then_b_text<'a>(input: &mut Pear<Text<'a>>) -> TResult<'a, char> {
    let a = eat('a')?;
    let _b = eat('b')?;
    Ok(a)
}

/// A parser that succeeds while consuming nothing.
#[parser]
fn empty_ok<I>(input: &mut Pear<I>) -> pear::input::Result<&'static str, I>
    where I: pear::input::Input
{
    let _ = input;
    Ok("empty")
}

//
// Short input #1: a delimiter `;` is present ("ax;rest" / "ax;").
// Both consumption strategies are exercised, plus the next parse start point.
//

#[test]
fn recovers_to_delimiter_without_consuming_it() {
    let mut input: Pear<Cursor<&str>> = Pear::new("ax;rest");

    let outcome = recover_until(
        &mut input, ';', RecoveryPoint::AtDelimiter, a_then_b
    ).unwrap();

    // The skipped range and the original error come back.
    let (skipped, error) = outcome.recovered().expect("recovered");
    assert_eq!(skipped, "x");
    assert_eq!(skipped.start, 1);
    assert_eq!(skipped.end, 2);
    assert_eq!(error.error.to_string(), "expected token 'b' but found 'x'");

    // The delimiter was preserved: the next parse starts on `;`.
    assert_eq!(pear::input::Input::token(&mut input), Some(';'));
    let rest = take_while(&mut input, |_| true).unwrap();
    assert_eq!(rest, ";rest");

    println!("[at-delimiter] skipped={:?} err=\"{}\" next=\"{}\"",
             skipped.values, error, rest.values);
}

#[test]
fn recovers_to_delimiter_consuming_it() {
    let mut input: Pear<Cursor<&str>> = Pear::new("ax;rest");

    let outcome = recover_until(
        &mut input, ';', RecoveryPoint::AfterDelimiter, a_then_b
    ).unwrap();

    let (skipped, error) = outcome.recovered().expect("recovered");
    assert_eq!(skipped, "x");
    assert_eq!(error.error.to_string(), "expected token 'b' but found 'x'");

    // The delimiter was consumed: the next parse starts right after `;`.
    let next = pear::input::Input::token(&mut input);
    assert_eq!(next, Some('r'));
    let rest = take_while(&mut input, |_| true).unwrap();
    assert_eq!(rest, "rest");

    println!("[after-delimiter] skipped={:?} err=\"{}\" next=\"{}\"",
             skipped.values, error, rest.values);
}

#[test]
fn recovery_when_delimiter_is_immediately_next() {
    // The failing parser consumes nothing before the delimiter: the skipped
    // range is zero-length, the position still advances correctly.
    let mut input: Pear<Cursor<&str>> = Pear::new(";rest");

    // `eat('b')` fails immediately on the very first token, the delimiter `;`:
    // skip 0 tokens, then hit `;`.
    let outcome = recover_until(
        &mut input, ';', RecoveryPoint::AfterDelimiter,
        |i| eat(i, 'b')
    ).unwrap();

    let (skipped, _) = outcome.recovered().expect("recovered");
    assert_eq!(skipped.len(), 0);
    assert_eq!(skipped.values, "");

    let rest = take_while(&mut input, |_| true).unwrap();
    assert_eq!(rest, "rest");

    println!("[zero-length-skip] skipped_len={} next=\"{}\"",
             skipped.len(), rest.values);
}

//
// Zero-length *success*: the success branch must not scan for the delimiter.
//

#[test]
fn zero_length_success_does_not_scan() {
    let mut input: Pear<Cursor<&str>> = Pear::new(";abc");

    let outcome = recover_until(
        &mut input, ';', RecoveryPoint::AfterDelimiter, empty_ok
    ).unwrap();

    assert!(!outcome.is_recovered());
    assert_eq!(outcome.parsed(), Some("empty"));

    // Nothing was scanned and nothing consumed: the `;` is still present.
    let all = take_while(&mut input, |_| true).unwrap();
    assert_eq!(all, ";abc");

    println!("[zero-length-success] input untouched: \"{}\"", all.values);
}

//
// Short input #2: EOF, no delimiter ("ax"): recovery must be reported as
// unrecoverable, with the original error retained. Both strategies agree.
//

#[test]
fn eof_without_delimiter_is_unrecoverable() {
    for point in [RecoveryPoint::AtDelimiter, RecoveryPoint::AfterDelimiter] {
        let mut input: Pear<Cursor<&str>> = Pear::new("ax");

        let err = recover_until(
            &mut input, ';', point, a_then_b
        ).expect_err("EOF without a delimiter is unrecoverable");

        // The new error flags unrecoverability and keeps the original message.
        let text = err.error.to_string();
        assert!(text.starts_with("unrecoverable parse error"), "got: {}", text);
        assert!(text.contains("expected token 'b' but found 'x'"), "got: {}", text);

        // The original failing parser is retained in the stack.
        assert!(err.stack.iter().any(|f| f.parser.name == "eat"));

        // The input is exhausted by the scan.
        assert!(!pear::input::Input::has(&mut input, 1));

        println!("[eof {:?}] unrecoverable error:\n{}", point, err);
    }
}

//
// No infinite loop on zero consumption: a recovery-driven loop where each
// iteration consumes the preserved delimiter terminates rather than spinning.
//

#[test]
fn repeated_recovery_makes_forward_progress() {
    // Every round preserves the delimiter (`AtDelimiter`); the surrounding
    // grammar then consumes it, so the loop cannot spin on zero consumption.
    let mut input: Pear<Cursor<&str>> = Pear::new("ax;b?");
    let mut recovered = 0;
    let mut iterations = 0;

    loop {
        iterations += 1;
        assert!(iterations <= 16, "recovery loop did not terminate");

        match recover_until(&mut input, ';', RecoveryPoint::AtDelimiter, a_then_b) {
            Ok(Recovery::Parsed(_)) => break,
            Ok(Recovery::Recovered { .. }) => {
                recovered += 1;
                // The delimiter is preserved at the failure point; the grammar
                // consumes it, so every round makes non-zero progress.
                eat(&mut input, ';').unwrap();
            }
            Err(_) => break,
        }
    }

    assert_eq!(recovered, 1);
    assert!(!pear::input::Input::has(&mut input, 1));
    println!("[forward-progress] terminated after {} iteration(s), {} recovery",
             iterations, recovered);
}

//
// The same scenario over `Text` to confirm span/context-style inputs work.
//

#[test]
fn works_over_text_input() {
    // Delimiter present, preserved: next parse begins on the newline.
    let mut input: Pear<Text<'_>> = Pear::from(Text::from("ax\nrest"));
    let outcome = recover_until(
        &mut input, '\n', RecoveryPoint::AtDelimiter, a_then_b_text
    ).unwrap();

    let (skipped, error) = outcome.recovered().expect("recovered");
    assert_eq!(skipped, "x");
    assert_eq!(error.error.to_string(), "expected token 'b' but found 'x'");
    assert_eq!(pear::input::Input::token(&mut input), Some('\n'));

    // EOF without the delimiter is unrecoverable over `Text` too.
    let mut input: Pear<Text<'_>> = Pear::from(Text::from("ax"));
    let err = recover_until(
        &mut input, '\n', RecoveryPoint::AfterDelimiter, a_then_b_text
    ).expect_err("unrecoverable");
    assert!(err.error.to_string().starts_with("unrecoverable parse error"));

    println!("[text] skipped=\"{}\" eof-err=\"{}\"", skipped, err.error);
}

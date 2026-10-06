use pear::input::{Input, Pear, Text};
use pear::combinators::{consumption, series, Consumption};
use pear::{macros::*, parsers::*};

type Result<'a, T> = pear::input::Result<T, Text<'a>>;

/// Eats the two-byte character 'é', then fails on the next token. The failure
/// is committed: nothing is rewound.
#[parser]
fn multibyte_then_fail<'a>(input: &mut Pear<Text<'a>>) -> Result<'a, char> {
    eat('é')?;
    eat('x')?
}

#[parser]
fn word<'a>(input: &mut Pear<Text<'a>>) -> Result<'a, &'a str> {
    take_some_while(|c: &char| c.is_alphabetic())?
}

/// Combination example: `consumption` nested inside a parser that skips
/// surrounding whitespace, composing with `skip_while` and `word`.
#[parser]
fn padded_word<'a>(input: &mut Pear<Text<'a>>) -> Result<'a, (Consumption<usize>, &'a str)> {
    skip_while(|c: &char| c.is_whitespace())?;
    let (inner, parsed) = consumption(input, |i| word(i));
    let parsed = parsed?;
    skip_while(|c: &char| c.is_whitespace())?;
    Ok((inner, parsed))
}

/// Combination example: `consumption` around a `series` of `word`s.
#[parser]
fn word_list<'a>(input: &mut Pear<Text<'a>>) -> Result<'a, Vec<&'a str>> {
    let words: Vec<&'a str> = series(|i| word(i), ',')?;
    Ok(words)
}

#[test]
fn committed_failure_after_multibyte_reports_real_cursor() {
    let string = "éy";
    let mut input: Pear<Text> = Pear::new(string);
    let (report, result) = consumption(&mut input, |i| multibyte_then_fail(i));

    assert!(result.is_err());
    // 'é' is two UTF-8 bytes and was really consumed before the failure: the
    // end cursor is 2, not 0 (unconsumed) and not 1 (the middle of 'é').
    assert_eq!(report.start, 0);
    assert_eq!(report.end, 'é'.len_utf8());
    assert_eq!(report.end, 2);
    assert!(string.is_char_boundary(report.start));
    assert!(string.is_char_boundary(report.end));
    // The consumption is committed: the input was not rewound.
    assert_eq!(input.token(), Some('y'));
}

#[test]
fn success_runs_parser_exactly_once() {
    let runs = std::cell::Cell::new(0usize);
    let string = "héllo";
    let mut input: Pear<Text> = Pear::new(string);
    let (report, result) = consumption(&mut input, |i| {
        runs.set(runs.get() + 1);
        word(i)
    });

    // A normal successful parse executes the parser exactly once.
    assert_eq!(runs.get(), 1);
    assert_eq!(result.unwrap(), "héllo");
    assert_eq!(report, Consumption { start: 0, end: string.len() });
}

#[test]
fn combination_nested_report_with_padding() {
    let string = "  hé  ";
    let mut input: Pear<Text> = Pear::new(string);
    let (outer, result) = consumption(&mut input, |i| padded_word(i));

    let (inner, parsed) = result.unwrap();
    assert_eq!(parsed, "hé");
    // The inner report covers only the word: 'h' (1 byte) + 'é' (2 bytes).
    assert_eq!(inner, Consumption { start: 2, end: 5 });
    assert!(string.is_char_boundary(inner.start));
    assert!(string.is_char_boundary(inner.end));
    // The outer report covers everything, including surrounding whitespace.
    assert_eq!(outer, Consumption { start: 0, end: string.len() });
}

#[test]
fn combination_series_success_and_committed_failure() {
    // Success: the report spans the whole list.
    let string = "one,two,three";
    let mut input: Pear<Text> = Pear::new(string);
    let (report, result) = consumption(&mut input, |i| word_list(i));
    assert_eq!(result.unwrap(), vec!["one", "two", "three"]);
    assert_eq!(report, Consumption { start: 0, end: string.len() });

    // Committed failure: after the multi-byte 'é', a trailing separator is
    // consumed and the final item fails at EOF. Everything consumed so far is
    // reported as consumed.
    let string = "one,téo,";
    let mut input: Pear<Text> = Pear::new(string);
    let (report, result) = consumption(&mut input, |i| word_list(i));
    assert!(result.is_err());
    assert_eq!(report.start, 0);
    assert_eq!(report.end, string.len());
    assert!(string.is_char_boundary(report.end));
    assert_eq!(&string[..report.end], "one,téo,");
    assert_eq!(input.token(), None);
}

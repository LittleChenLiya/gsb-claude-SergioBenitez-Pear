use pear::input::{Input, Pear, Text};
use pear::macros::parser;
use pear::parsers::{eat, peek};
use pear::combinators::{recover_until, recover_until_with, Recovery, RecoveryOptions};

type Result<'a, T> = pear::input::Result<T, Text<'a>>;

/// Parses the two-token production `qx`. On inputs like "qr..." it consumes
/// `q` and then fails, exercising recovery's rewind of the partial production.
#[parser]
fn qx<'a>(input: &mut Pear<Text<'a>>) -> Result<'a, (char, char)> {
    let q = eat('q')?;
    let x = eat('x')?;
    Ok((q, x))
}

// Two short in-memory fixtures: one containing the `;` delimiter, one that
// runs into EOF without any delimiter.
const WITH_DELIMITER: &str = "qr;next";
const EOF_NO_DELIMITER: &str = "qrz";

#[test]
fn recovers_to_delimiter_keeping_it_by_default() {
    let mut input = Pear::new(Text::from(WITH_DELIMITER));
    let recovery = recover_until(&mut input, qx, ';').unwrap();

    let recovered = match recovery {
        Recovery::Recovered(recovered) => recovered,
        other => panic!("expected Recovered, got {:?}", other),
    };

    // The skipped range covers the whole failed production "qr"...
    assert_eq!(recovered.skipped.snippet, Some("qr"));
    assert_eq!(recovered.skipped.start, (1, 1, 0));
    assert_eq!(recovered.skipped.end, (1, 3, 2));
    assert_eq!(recovered.delimiter, ';');

    // ...and the original error from the failing parser is preserved.
    assert!(recovered.error.to_string().contains("expected token"));

    // Default strategy: the delimiter is NOT consumed, so the next parse
    // starts at ';'.
    assert_eq!(input.token(), Some(';'));
    eat(&mut input, ';').unwrap();
    assert_eq!(eat(&mut input, 'n').unwrap(), 'n');
}

#[test]
fn recovers_to_delimiter_consuming_it_with_options() {
    let mut input = Pear::new(Text::from(WITH_DELIMITER));
    let options = RecoveryOptions { consume_delimiter: true };
    let recovery = recover_until_with(&mut input, options, qx, ';').unwrap();

    match recovery {
        Recovery::Recovered(ref recovered) => {
            assert_eq!(recovered.skipped.snippet, Some("qr"));
            assert_eq!(recovered.delimiter, ';');
            assert!(recovered.error.to_string().contains("expected token"));
        }
        other => panic!("expected Recovered, got {:?}", other),
    }

    // Consume strategy: the next parse starts right after the delimiter.
    assert_eq!(input.token(), Some('n'));
    assert_eq!(eat(&mut input, 'n').unwrap(), 'n');
    assert_eq!(input.token(), Some('e'));
}

#[test]
fn eof_without_delimiter_is_unrecoverable() {
    let mut input = Pear::new(Text::from(EOF_NO_DELIMITER));
    let result = recover_until(&mut input, qx, ';');

    let error = result.unwrap_err();
    assert!(error.to_string().contains("unrecoverable"));

    // The input was scanned all the way to EOF looking for the delimiter.
    assert_eq!(input.token(), None);
}

#[test]
fn zero_length_success_does_not_scan() {
    // `peek` succeeds without consuming anything: a zero-length success.
    let mut input = Pear::new(Text::from(";abc"));
    let recovery = recover_until(&mut input, |i: &mut Pear<Text<'_>>| peek(i, ';'), ';');
    match recovery.unwrap() {
        Recovery::Parsed(()) => {}
        other => panic!("expected Parsed, got {:?}", other),
    }

    // The success branch never scans: the input is still at the first token.
    assert_eq!(input.token(), Some(';'));
}

#[test]
fn consuming_success_does_not_scan_either() {
    let mut input = Pear::new(Text::from("qx;abc"));
    let recovery = recover_until(&mut input, qx, ';').unwrap();
    match recovery {
        Recovery::Parsed(pair) => assert_eq!(pair, ('q', 'x')),
        other => panic!("expected Parsed, got {:?}", other),
    }

    // `qx` consumed exactly "qx"; the delimiter was never scanned for.
    assert_eq!(input.token(), Some(';'));
}

#[test]
fn repeated_recovery_terminates_on_empty_consumption() {
    let mut input = Pear::new(Text::from("qr;qs;qx;"));
    let options = RecoveryOptions { consume_delimiter: true };

    let (mut parsed, mut recovered, mut empty_skips) = (0, 0, 0);
    loop {
        match recover_until_with(&mut input, options, qx, ';') {
            Ok(Recovery::Parsed(_)) => parsed += 1,
            Ok(Recovery::Recovered(r)) => {
                recovered += 1;
                if r.skipped.snippet == Some("") {
                    // The parser failed without consuming and the delimiter
                    // was already current: zero tokens were skipped, yet the
                    // consumed delimiter guarantees progress.
                    empty_skips += 1;
                }
            }
            Err(error) => {
                assert!(error.to_string().contains("unrecoverable"));
                break;
            }
        }
    }

    assert_eq!(parsed, 1);      // "qx"
    assert_eq!(recovered, 3);   // "qr", "qs", and the trailing ";"
    assert_eq!(empty_skips, 1); // the trailing ";" skipped zero tokens
    assert_eq!(input.token(), None);
}

#[test]
fn demo_recovery_report() {
    let options = RecoveryOptions { consume_delimiter: true };
    for source in &[WITH_DELIMITER, EOF_NO_DELIMITER] {
        let mut input = Pear::new(Text::from(*source));
        match recover_until_with(&mut input, options, qx, ';') {
            Ok(Recovery::Parsed(value)) => {
                println!("{:?}: parsed {:?}", source, value);
            }
            Ok(Recovery::Recovered(r)) => {
                println!("{:?}: recovered; skipped {:?} spanning {:?}..{:?}, \
                          delimiter {:?}, original error: {}",
                    source, r.skipped.snippet, r.skipped.start, r.skipped.end,
                    r.delimiter, r.error);
            }
            Err(error) => {
                println!("{:?}: unrecoverable: {}", source, error);
            }
        }
    }
}

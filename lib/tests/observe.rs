use pear::error::Expected;
use pear::input::{Pear, Text, Cursor, Extent};
use pear::{macros::*, parsers::*};

/// The existing string input: units are `char`s.
type StrInput<'a> = Pear<Text<'a>>;

/// The existing byte input: units are `u8`s.
type ByteInput<'a> = Pear<Cursor<&'a [u8]>>;

/// A short regression example through the original interface: the observer
/// is hooked into the regular `#[parser]` + `parse!` data flow.
#[parser]
fn observed_word<'a>(
    input: &mut StrInput<'a>
) -> pear::input::Result<(&'a str, usize, usize), StrInput<'a>> {
    let before = observe(|_, n| Ok::<_, &'static str>(n))?;
    let word = take_while(|c: &char| c.is_alphabetic())?;
    let after = observe(|_, n| Ok::<_, &'static str>(n))?;
    (word, before, after)
}

#[test]
fn string_positions_are_observed_in_chars() {
    let mut input = StrInput::new("héllo");

    // Before: at the start, nothing consumed. The unit is a `char`.
    let before = observe(&mut input, |t, n| Ok::<_, &'static str>((t, n))).unwrap();
    assert_eq!(before, (Some('h'), 0));

    // Observing does not advance: a second observation sees the same position.
    let again = observe(&mut input, |t, n| Ok::<_, &'static str>((t, n))).unwrap();
    assert_eq!(again, before);

    // And parsing still sees the same tokens: nothing was consumed.
    eat(&mut input, 'h').unwrap();
    eat(&mut input, 'é').unwrap();

    // After: two units consumed. Units are `char`s, not bytes: the
    // multibyte `é` counts as one, so this is 2, not 3.
    let after = observe(&mut input, |t, n| Ok::<_, &'static str>((t, n))).unwrap();
    assert_eq!(after, (Some('l'), 2));

    // Subsequent parsing is unaffected by the observations.
    let rest = take_while(&mut input, |_| true).unwrap();
    assert_eq!(rest, "llo");
    eof(&mut input).unwrap();
}

#[test]
fn byte_positions_are_observed_in_bytes() {
    let bytes: &[u8] = "héllo".as_bytes();
    let mut input = ByteInput::new(bytes);

    // Before: at the start. The unit is a `u8`.
    let before = observe(&mut input, |t, n| Ok::<_, &'static str>((t, n))).unwrap();
    assert_eq!(before, (Some(b'h'), 0));

    // Observing does not advance: parsing still sees the same bytes.
    eat(&mut input, b'h').unwrap();
    eat_slice(&mut input, b"\xC3\xA9").unwrap();

    // After: three units consumed. Units are bytes, so the UTF-8 encoding
    // of `é` counts as two: the same visible content as the string input
    // above consumes 3 units here, not 2.
    let after = observe(&mut input, |t, n| Ok::<_, &'static str>((t, n))).unwrap();
    assert_eq!(after, (Some(b'l'), 3));

    // Subsequent parsing is unaffected by the observations.
    let rest = take_while(&mut input, |_| true).unwrap();
    assert_eq!(rest, Extent { start: 3, end: 6, values: &b"llo"[..] });
    eof(&mut input).unwrap();
}

#[test]
fn observer_failure_becomes_parse_error_with_cause() {
    let mut input = StrInput::new("héllo");

    let result = observe(&mut input, |_, _| Err::<(), _>("boom: bad position"));
    let error = result.unwrap_err();

    // The observer's failure is carried by the parse error as its cause.
    match &error.error {
        Expected::Other(message) => assert_eq!(&**message, "boom: bad position"),
        other => panic!("unexpected error: {:?}", other),
    }

    assert!(error.to_string().contains("boom: bad position"));

    // The failed observation did not advance the input.
    let position = observe(&mut input, |t, n| Ok::<_, &'static str>((t, n))).unwrap();
    assert_eq!(position, (Some('h'), 0));

    // And parsing still succeeds afterwards.
    let all = take_while(&mut input, |_| true).unwrap();
    assert_eq!(all, "héllo");
    eof(&mut input).unwrap();
}

#[test]
fn regression_example_through_parse_macro() {
    // The same observations, through the original `parse!` interface.
    let (word, before, after) = parse!(observed_word: StrInput::new("héllo")).unwrap();
    assert_eq!(word, "héllo");
    assert_eq!(before, 0);
    assert_eq!(after, 5);
}

/// Small in-memory fixture demonstrating the observation data flow. Run
/// with `cargo test -p pear --test observe -- --nocapture` to see the log.
#[test]
fn demo_observation_log() {
    let mut string = StrInput::new("héllo");
    let mut bytes = ByteInput::new("héllo".as_bytes());

    let s0 = observe(&mut string, |t, n| Ok::<_, &'static str>((t, n))).unwrap();
    let b0 = observe(&mut bytes, |t, n| Ok::<_, &'static str>((t, n))).unwrap();
    println!("before: string={:?} bytes={:?}", s0, b0);

    eat(&mut string, 'h').unwrap();
    eat(&mut string, 'é').unwrap();
    eat(&mut bytes, b'h').unwrap();
    eat_slice(&mut bytes, b"\xC3\xA9").unwrap();

    let s1 = observe(&mut string, |t, n| Ok::<_, &'static str>((t, n))).unwrap();
    let b1 = observe(&mut bytes, |t, n| Ok::<_, &'static str>((t, n))).unwrap();
    println!("after:  string={:?} bytes={:?}", s1, b1);

    let error = observe(&mut string, |_, _| Err::<(), _>("boom")).unwrap_err();
    println!("cause:  {}", error);

    assert_eq!(s0, (Some('h'), 0));
    assert_eq!(b0, (Some(b'h'), 0));
    assert_eq!(s1, (Some('l'), 2));
    assert_eq!(b1, (Some(b'l'), 3));
    assert!(error.to_string().contains("boom"));
}

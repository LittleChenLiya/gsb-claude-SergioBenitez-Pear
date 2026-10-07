use pear::{macros::*, parsers::*};
use pear::input::{Input, Pear, Text, Cursor, Position};

type TextInput<'a> = Pear<Text<'a>>;
type TextResult<'a, T> = pear::input::Result<T, TextInput<'a>>;

type ByteInput<'a> = Pear<Cursor<&'a [u8]>>;
type ByteResult<'a, T> = pear::input::Result<T, ByteInput<'a>>;

fn ok<I: Input>(_: &Position<I>) -> Result<(), &'static str> { Ok(()) }

// Observes the position before and after eating a multibyte UTF-8 token.
// Observing must not advance the input: the final `eat` and `eof` (run by
// `parse!`) still succeed.
#[parser]
fn utf8_positions<'a>(
    input: &mut TextInput<'a>
) -> TextResult<'a, (Position<Text<'a>>, Position<Text<'a>>, Position<Text<'a>>, char)> {
    let before = position(ok)?;
    eat('h')?;
    eat('í')?; // 2-byte UTF-8 char
    let mid = position(ok)?;
    let mid_again = position(ok)?; // repeated observation: non-advancing
    let last = eat('!')?;
    (before, mid, mid_again, last)
}

// Same idea for a byte stream: units are `u8`, consumed counts bytes.
#[parser]
fn byte_positions<'a>(
    input: &mut ByteInput<'a>
) -> ByteResult<'a, (Position<Cursor<&'a [u8]>>, Position<Cursor<&'a [u8]>>, u8)> {
    let before = position(ok)?;
    eat_slice(&b"ab"[..])?;
    let mid = position(ok)?;
    let last = eat(b'!')?;
    (before, mid, last)
}

// The observer rejects the position; its failure becomes the error's cause.
#[parser]
fn guarded<'a>(input: &mut TextInput<'a>) -> TextResult<'a, char> {
    eat('a')?;
    position(|pos: &Position<Text<'a>>| {
        if pos.consumed >= 10 { Ok(()) } else { Err("too early in input") }
    })?;
    let last = eat('b')?;
    last
}

// Regression for the original interface: markers, context, and regular
// parsers behave as before, and `Position::consumed` agrees with the
// original `parse_current_marker!()` data flow.
#[parser]
fn marker_regression<'a>(input: &mut TextInput<'a>) -> TextResult<'a, (usize, usize, usize, usize)> {
    let m0 = parse_current_marker!();
    let p0 = position(ok)?;
    eat_slice("ab")?;
    let m1 = parse_current_marker!();
    let p1 = position(ok)?;
    eat('c')?;
    (m0, p0.consumed, m1, p1.consumed)
}

#[test]
fn utf8_position_observation() {
    let (before, mid, mid_again, last) = parse!(utf8_positions: TextInput::new("hí!")).unwrap();

    // Units are `char`s; consumption is measured in UTF-8 bytes.
    assert_eq!(before.unit, Some('h'));
    assert_eq!(before.consumed, 0);

    assert_eq!(mid.unit, Some('!'));
    assert_eq!(mid.consumed, 3); // 'h' (1 byte) + 'í' (2 bytes)

    // Observing is read-only: repeated observation is identical...
    assert_eq!(mid, mid_again);

    // ...and the input was never advanced, so parsing continued normally.
    assert_eq!(last, '!');
}

#[test]
fn bytes_position_observation() {
    let (before, mid, last) = parse!(byte_positions: ByteInput::new(&b"ab!"[..])).unwrap();

    // Units are `u8` bytes; consumption counts bytes.
    assert_eq!(before.unit, Some(b'a'));
    assert_eq!(before.consumed, 0);

    assert_eq!(mid.unit, Some(b'!'));
    assert_eq!(mid.consumed, 2);

    assert_eq!(last, b'!');
}

#[test]
fn observer_failure_becomes_cause() {
    let error = parse!(guarded: TextInput::new("ab")).unwrap_err();

    // The observer's failure is carried as the error's cause.
    let cause = error.cause.as_ref().expect("observer failure sets a cause");
    assert_eq!(&**cause, "too early in input");

    // ...and shows up in the displayed error.
    let displayed = format!("{}", error);
    assert!(displayed.contains("position observer failed"), "{}", displayed);
    assert!(displayed.contains("caused by: too early in input"), "{}", displayed);
}

#[test]
fn original_interface_regression() {
    // The original marker data flow is unchanged and agrees with `consumed`.
    let (m0, c0, m1, c1) = parse!(marker_regression: TextInput::new("abc")).unwrap();
    assert_eq!((m0, c0), (0, 0));
    assert_eq!((m1, c1), (2, 2));

    // Existing parsers are unaffected: errors from before still carry no cause.
    let error = parse!(guarded: TextInput::new("b")).unwrap_err();
    assert!(error.cause.is_none());
}

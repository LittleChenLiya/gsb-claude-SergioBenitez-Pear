use crate::input::{Pear, Input, Rewind, Token, Result, ParserInfo};
use crate::macros::parser;
use crate::parsers::*;

pub trait Collection<A>: Default + Extend<A> {
    #[inline(always)]
    fn push(&mut self, item: A) {
        self.extend(Some(item))
    }
}

impl<A, T: Default + Extend<A>> Collection<A> for T {  }

/// Parses `p`, returning `Some` if it succeeds and `None` if it fails. Discards
/// the error message.
pub fn ok<I, P, O>(input: &mut Pear<I>, p: P) -> Option<O>
    where I: Input, P: FnOnce(&mut Pear<I>) -> Result<O, I>
{
    let save = input.emit_error;
    input.emit_error = false;
    let ok = p(input).ok();
    input.emit_error = save;
    ok
}

/// Parses `p`, returning `true` if it succeeds and `false` if it fails.
/// Discards the error message.
pub fn succeeds<I, P, O>(input: &mut Pear<I>, p: P) -> bool
    where I: Input, P: FnOnce(&mut Pear<I>) -> Result<O, I>
{
    ok(input, p).is_some()
}

/// Parses `p` until `p` fails, returning the last successful `p`.
#[parser(raw)]
pub fn last_of_many<I, O, P>(input: &mut Pear<I>, mut p: P) -> Result<O, I>
    where I: Input, P: FnMut(&mut Pear<I>) -> Result<O, I>
{
    loop {
        let output = p()?;
        if succeeds(input, eof) {
            return Ok(output);
        }
    }
}

/// Skips all tokens that match `f` before and after a `p`, returning `p`.
#[parser(raw)]
pub fn surrounded<I, O, F, P>(input: &mut Pear<I>, mut p: P, mut f: F) -> Result<O, I>
    where I: Input,
          F: FnMut(&I::Token) -> bool,
          P: FnMut(&mut Pear<I>) -> Result<O, I>
{
    skip_while(&mut f)?;
    let output = p()?;
    skip_while(&mut f)?;
    Ok(output)
}

/// Parses as many `p` as possible until EOF is reached, collecting them into a
/// `C`. Fails if `p` every fails. `C` may be empty.
#[parser(raw)]
pub fn collect<C, I, O, P>(input: &mut Pear<I>, mut p: P) -> Result<C, I>
    where C: Collection<O>, I: Input, P: FnMut(&mut Pear<I>) -> Result<O, I>
{
    let mut collection = C::default();
    loop {
        if succeeds(input, eof) {
            return Ok(collection);
        }

        collection.push(p()?);
    }
}

/// Parses as many `p` as possible until EOF is reached, collecting them into a
/// `C`. Fails if `p` ever fails. `C` is not allowed to be empty.
#[parser(raw)]
pub fn collect_some<C, I, O, P>(input: &mut Pear<I>, mut p: P) -> Result<C, I>
    where C: Collection<O>, I: Input, P: FnMut(&mut Pear<I>) -> Result<O, I>
{
    let mut collection = C::default();
    loop {
        collection.push(p()?);
        if succeeds(input, eof) {
            return Ok(collection);
        }
    }
}

/// Parses as many `p` as possible until EOF is reached or `p` fails, collecting
/// them into a `C`. `C` may be empty.
#[parser(raw)]
pub fn try_collect<C, I, O, P>(input: &mut Pear<I>, mut p: P) -> Result<C, I>
    where C: Collection<O>, I: Input + Rewind, P: FnMut(&mut Pear<I>) -> Result<O, I>
{
    let mut collection = C::default();
    loop {
        if succeeds(input, eof) {
            return Ok(collection);
        }

        // FIXME: We should be able to call `parse_marker!` here.
        let start = input.mark(&crate::input::ParserInfo {
            name: "try_collect",
            raw: true
        });

        match ok(input, |i| p(i)) {
            Some(val) => collection.push(val),
            None => {
                input.rewind_to(start);
                break;
            }
        }
    }

    Ok(collection)
}

/// Parses many `separator` delimited `p`s, the entire collection of which must
/// start with `start` and end with `end`. `item` Gramatically, this is:
///
/// START (item SEPERATOR)* END
#[parser(raw)]
pub fn delimited_collect<C, I, T, S, O, P>(
    input: &mut Pear<I>,
    start: T,
    mut item: P,
    separator: S,
    end: T,
) -> Result<C, I>
    where C: Collection<O>,
          I: Input,
          T: Token<I> + Clone,
          S: Into<Option<T>>,
          P: FnMut(&mut Pear<I>) -> Result<O, I>,
{
    eat(start)?;

    let seperator = separator.into();
    let mut collection = C::default();
    loop {
        if succeeds(input, |i| eat(i, end.clone())) {
            break;
        }

        collection.push(item()?);

        if let Some(ref separator) = seperator {
            if !succeeds(input, |i| eat(i, separator.clone())) {
                eat(end.clone())?;
                break;
            }
        }
    }

    Ok(collection)
}

/// Parses many `separator` delimited `p`s. Gramatically, this is:
///
/// item (SEPERATOR item)*
#[parser(raw)]
pub fn series<C, I, S, O, P>(
    input: &mut Pear<I>,
    mut item: P,
    seperator: S,
) -> Result<C, I>
    where C: Collection<O>,
          I: Input,
          S: Token<I> + Clone,
          P: FnMut(&mut Pear<I>) -> Result<O, I>,
{
    let mut collection = C::default();
    loop {
        collection.push(item()?);
        if !succeeds(input, |i| eat(i, seperator.clone())) {
            break;
        }
    }

    Ok(collection)
}

/// Parses many `separator` delimited `p`s with an optional trailing separator.
/// Gramatically, this is:
///
/// item (SEPERATOR item)* SEPERATOR?
#[parser(raw)]
pub fn trailing_series<C, I, S, O, P>(
    input: &mut Pear<I>,
    mut item: P,
    seperator: S,
) -> Result<C, I>
    where C: Collection<O>,
          I: Input,
          S: Token<I> + Clone,
          P: FnMut(&mut Pear<I>) -> Result<O, I>,
{
    let mut collection = C::default();
    let mut have_some = false;
    loop {
        if have_some {
            if let Some(item) = ok(input, |i| item(i)) {
                collection.push(item);
            } else {
                break
            }
        } else {
            collection.push(item()?);
            have_some = true;
        }

        if !succeeds(input, |i| eat(i, seperator.clone())) {
            break;
        }
    }

    Ok(collection)
}

/// Parses many `separator` delimited `p`s that are collectively prefixed with
/// `prefix`. Gramatically, this is:
///
/// PREFIX (item SEPERATOR)*
#[parser(raw)]
pub fn prefixed_series<C, I, T, O, P>(
    input: &mut Pear<I>,
    prefix: T,
    item: P,
    seperator: T,
) -> Result<C, I>
    where C: Collection<O>,
          I: Input,
          T: Token<I> + Clone,
          P: FnMut(&mut Pear<I>) -> Result<O, I>,
{
    if !succeeds(input, |i| eat(i, prefix)) {
        return Ok(C::default());
    }

    series(input, item, seperator)
}

/// A report of the input positions that a single parser invocation actually
/// consumed, together with its result.
///
/// Produced by [`consumed()`]. The parser is executed exactly once: `start` is
/// the cursor immediately before it ran, and `end` is the cursor where the
/// input stood immediately after it finished. On a committed failure the input
/// that was already consumed is still reflected in `end`; it is never reported
/// as unconsumed. For UTF-8 inputs such as [`Text`](crate::input::Text) both
/// cursors are byte offsets that always lie on a character boundary.
///
/// `context` spans the input between `start` and `end`; `result` is the value
/// or error returned by the parser.
pub struct Consumption<I: Input, O> {
    /// The cursor at which the parser started.
    pub start: I::Marker,
    /// The cursor at which the parser ended, including input consumed before a
    /// committed failure.
    pub end: I::Marker,
    /// The input spanning from `start` up to but excluding `end`.
    pub context: I::Context,
    /// The result of the single parser invocation.
    pub result: std::result::Result<O, crate::input::ParseError<I>>,
}

impl<I: Input, O> Consumption<I, O> {
    /// Returns `true` if the reported parser invocation succeeded.
    pub fn is_ok(&self) -> bool {
        self.result.is_ok()
    }

    /// Returns `true` if the reported parser invocation failed after consuming
    /// at least one token, that is, the failure was _committed_.
    pub fn is_committed_failure(&self) -> bool
        where I::Marker: PartialEq
    {
        self.result.is_err() && self.start != self.end
    }
}

/// Runs `p` exactly once and reports the true start and end cursors alongside
/// its result.
///
/// Unlike a parser attribute with `rewind`, this neither rewinds the input nor
/// retries `p`: it is invoked a single time, so its side effects occur once,
/// and any input consumed before a failure stays consumed and is reflected in
/// the report's `end` cursor. No recovery strategy is applied; the original
/// result of `p` is passed through unchanged in [`Consumption::result`].
///
/// # Examples
///
/// On success the cursors bound exactly the consumed slice:
///
/// ```rust
/// use pear::input::{Pear, Text};
/// use pear::combinators::consumed;
/// use pear::parsers::take_while;
///
/// let mut input = Pear::from(Text::from("αβγ.."));
/// let report = consumed(&mut input, |i| take_while(i, |c| c.is_alphabetic()));
/// assert!(report.is_ok());
/// assert_eq!(report.start, 0);
/// assert_eq!(report.end, 6); // three multibyte chars, two UTF-8 bytes each
/// assert_eq!(report.context.snippet, Some("αβγ"));
/// assert_eq!(report.result.unwrap(), "αβγ");
/// ```
///
/// After a multibyte character is consumed and the parser then commits to a
/// failure, `end` is the real cursor past that character and never lands in the
/// middle of its bytes:
///
/// ```rust
/// use pear::input::{Pear, Text};
/// use pear::combinators::consumed;
/// use pear::parsers::{eat, eof};
///
/// let source = "中x";
/// let mut input = Pear::from(Text::from(source));
/// let report = consumed(&mut input, |i| {
///     eat(i, '中')?;
///     eof(i)?;
///     Ok::<(), _>(())
/// });
///
/// assert!(report.result.is_err());
/// assert_eq!(report.start, 0);
/// assert_eq!(report.end, '中'.len_utf8());
/// assert_eq!(report.context.snippet, Some("中"));
/// // The end cursor is a valid UTF-8 boundary, not a position inside a byte:
/// // slicing the source at it succeeds and yields the consumed prefix.
/// assert_eq!(source.get(..report.end), Some("中"));
/// ```
pub fn consumed<I, P, O>(input: &mut Pear<I>, p: P) -> Consumption<I, O>
    where I: Input, P: FnOnce(&mut Pear<I>) -> Result<O, I>
{
    let info = ParserInfo { name: "consumed", raw: true };
    let start = input.mark(&info);
    let result = p(input);
    let end = input.mark(&info);
    let context = input.context(start);
    Consumption { start, end, context, result }
}

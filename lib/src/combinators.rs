use crate::error::Expected;
use crate::input::{Pear, Input, Rewind, Token, Result, ParseError};
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

/// Options controlling how [`recover_until_with()`] treats the delimiter once
/// it is found.
///
/// The default (used by [`recover_until()`]) leaves the delimiter in the
/// input, so existing call sites and follow-up parsers observe the delimiter
/// exactly as if no recovery had been configured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecoveryOptions {
    /// Whether to consume the delimiter token after recovering. When `false`
    /// (the default), the delimiter is left as the current token.
    pub consume_delimiter: bool,
}

/// Information about a parse failure that was recovered from by skipping
/// input up to a delimiter.
pub struct Recovered<I: Input> {
    /// The input range that was skipped: from the position at which the
    /// failing parser was invoked up to (but excluding) the delimiter.
    pub skipped: I::Context,
    /// The original error emitted by the failing parser.
    pub error: ParseError<I>,
    /// The delimiter token that terminated the recovery scan.
    pub delimiter: I::Token,
}

impl<I: Input> std::fmt::Debug for Recovered<I>
    where I::Context: std::fmt::Debug,
          I::Token: std::fmt::Debug,
          I::Slice: std::fmt::Debug
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recovered")
            .field("skipped", &self.skipped)
            .field("error", &self.error)
            .field("delimiter", &self.delimiter)
            .finish()
    }
}

/// The result of a [`recover_until()`] or [`recover_until_with()`] parse.
pub enum Recovery<O, I: Input> {
    /// The parser succeeded. No recovery scan was performed.
    Parsed(O),
    /// The parser failed; input was skipped up to the delimiter.
    Recovered(Recovered<I>),
}

impl<O: std::fmt::Debug, I: Input> std::fmt::Debug for Recovery<O, I>
    where I::Context: std::fmt::Debug,
          I::Token: std::fmt::Debug,
          I::Slice: std::fmt::Debug
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Recovery::Parsed(output) => f.debug_tuple("Parsed").field(output).finish(),
            Recovery::Recovered(recovered) => {
                f.debug_tuple("Recovered").field(recovered).finish()
            }
        }
    }
}

/// Parses `p`. If `p` fails, rewinds to the position at which `p` was invoked
/// and skips tokens until `delimiter` is the current token, then returns
/// [`Recovery::Recovered`] with the skipped range and the original error. The
/// delimiter is _not_ consumed; use [`recover_until_with()`] to change this.
///
/// If `p` succeeds, returns [`Recovery::Parsed`] without scanning any input.
/// If EOF is reached before `delimiter` is found, the parse is unrecoverable
/// and an error is returned.
///
/// Recovery performs a single forward scan and never retries `p`, so it
/// always terminates, even if `p` fails without consuming any input.
pub fn recover_until<I, T, O, P>(
    input: &mut Pear<I>,
    p: P,
    delimiter: T,
) -> Result<Recovery<O, I>, I>
    where I: Input + Rewind,
          T: Token<I>,
          P: FnMut(&mut Pear<I>) -> Result<O, I>
{
    recover_until_with(input, RecoveryOptions::default(), p, delimiter)
}

/// Like [`recover_until()`] but with explicit `options` controlling whether
/// the delimiter token is consumed after recovery.
#[parser(raw)]
pub fn recover_until_with<I, T, O, P>(
    input: &mut Pear<I>,
    options: RecoveryOptions,
    mut p: P,
    delimiter: T,
) -> Result<Recovery<O, I>, I>
    where I: Input + Rewind,
          T: Token<I>,
          P: FnMut(&mut Pear<I>) -> Result<O, I>
{
    let start = parse_current_marker!();
    match p(input) {
        Ok(output) => Ok(Recovery::Parsed(output)),
        Err(error) => {
            // Discard the failed production entirely, then scan forward to
            // the delimiter. `skip` consumes one token per step, so the scan
            // always terminates, even when `p` consumed nothing at all.
            input.rewind_to(start);
            input.skip(|t| &delimiter != t);
            match input.token() {
                Some(token) if delimiter == token => {
                    let skipped = input.context(start);
                    if options.consume_delimiter {
                        input.eat(|t| &delimiter == t);
                    }

                    Ok(Recovery::Recovered(Recovered { skipped, error, delimiter: token }))
                }
                _ => return parse_error!(Expected::from(
                    "unrecoverable parse failure: delimiter not found before EOF"))
            }
        }
    }
}

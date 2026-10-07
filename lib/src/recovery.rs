//! Explicit parser error recovery.
//!
//! This module is opt-in via the `recovery` feature. It provides the
//! [`recover_until()`] combinator, which runs an inner parser and, on failure,
//! explicitly _recovers_ by scanning forward to a delimiter instead of
//! propagating the error. The skipped range and the original parse error are
//! both returned so callers can report diagnostics while continuing to parse.
//!
//! [`recover_until()`]: crate::recovery::recover_until

use crate::input::{Input, Pear, Result, Rewind, Token};
use crate::macros::parser;

/// Selects where the input is positioned after a successful recovery.
///
/// * [`RecoveryPoint::AtDelimiter`]: the delimiter is detected but **left
///   unconsumed**; the next parse starts on the delimiter itself.
/// * [`RecoveryPoint::AfterDelimiter`]: the delimiter is **consumed**; the
///   next parse starts just after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryPoint {
    /// Do not consume the delimiter. The next parser starts pointing at it.
    AtDelimiter,
    /// Consume the delimiter. The next parser starts just past it.
    AfterDelimiter,
}

impl Default for RecoveryPoint {
    /// The default is [`RecoveryPoint::AtDelimiter`]: recovery never hides the
    /// delimiter from the surrounding grammar.
    fn default() -> Self {
        RecoveryPoint::AtDelimiter
    }
}

/// The outcome of [`recover_until()`].
pub enum Recovery<O, I: Input> {
    /// The inner parser succeeded without engaging the recovery machinery. The
    /// input was never scanned for the delimiter.
    Parsed(O),
    /// The inner parser failed, the delimiter was found, and parsing recovered.
    /// The skipped tokens (the range between the failure position and the
    /// delimiter) and the original error are returned verbatim.
    Recovered {
        /// The tokens skipped while scanning from the failure to the delimiter.
        skipped: I::Many,
        /// The original error produced by the inner parser.
        error: crate::input::ParseError<I>,
    },
}

impl<O, I: Input> std::fmt::Debug for Recovery<O, I>
    where O: std::fmt::Debug, I::Many: std::fmt::Debug
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Recovery::Parsed(value) => f.debug_tuple("Parsed").field(value).finish(),
            Recovery::Recovered { skipped, error } => f
                .debug_struct("Recovered")
                .field("skipped", skipped)
                // `ParseError` only needs `Show`, which every input provides;
                // avoid requiring `Debug` on token/slice/context here.
                .field("error", &format_args!("{}", error))
                .finish(),
        }
    }
}

impl<O, I: Input> Recovery<O, I> {
    /// Returns `true` if the inner parser failed but recovery succeeded.
    #[inline]
    pub fn is_recovered(&self) -> bool {
        matches!(self, Recovery::Recovered { .. })
    }

    /// Returns the successful output, discarding any recovery information.
    #[inline]
    pub fn parsed(self) -> Option<O> {
        match self {
            Recovery::Parsed(value) => Some(value),
            Recovery::Recovered { .. } => None,
        }
    }

    /// Returns the skipped range and original error if recovery occurred.
    #[inline]
    pub fn recovered(self) -> Option<(I::Many, crate::input::ParseError<I>)> {
        match self {
            Recovery::Parsed(_) => None,
            Recovery::Recovered { skipped, error } => Some((skipped, error)),
        }
    }
}

/// Runs parser `p`, returning [`Recovery::Parsed`] with its output on success.
///
/// If `p` fails, explicit error recovery is attempted: tokens are skipped from
/// the failure position up to `delimiter`. If the delimiter is found, the
/// combinator succeeds with [`Recovery::Recovered`], carrying the skipped
/// range and the _original_ error. Whether the delimiter itself is consumed is
/// selected by `point`:
///
/// * [`RecoveryPoint::AtDelimiter`] (the default via
///   [`RecoveryPoint::default()`]): the delimiter is not consumed, so the next
///   parse starts on it.
/// * [`RecoveryPoint::AfterDelimiter`]: the delimiter is consumed, so the next
///   parse starts immediately after it.
///
/// # Guarantees
///
/// * **The successful branch never scans.** If `p` succeeds, the rest of the
///   input (including any delimiter) is untouched.
/// * **EOF is unrecoverable.** If the delimiter cannot be found before the end
///   of the input, the combinator returns an `Err` that reports the failure as
///   unrecoverable and retains the original error in its parser stack.
/// * **No zero-consumption looping.** Scanning advances token by token, so the
///   combinator itself never spins without consuming input. Under
///   `AfterDelimiter`, finding a delimiter always advances past it; under
///   `AtDelimiter`, the surrounding grammar is expected to consume the
///   preserved delimiter to make progress.
#[parser(raw)]
pub fn recover_until<I, T, P, O>(
    input: &mut Pear<I>,
    delimiter: T,
    point: RecoveryPoint,
    mut p: P,
) -> Result<Recovery<O, I>, I>
    where I: Input + Rewind,
          T: Token<I>,
          P: FnMut(&mut Pear<I>) -> Result<O, I>
{
    match p(input) {
        // The success branch never scans for a delimiter.
        Ok(value) => Ok(Recovery::Parsed(value)),
        Err(error) => {
            // Skip every token up to (but not including) the delimiter. The
            // underlying `take` consumes whole tokens, so scanning always makes
            // progress and cannot spin on a zero-length match.
            let skipped = input.take(|t| &delimiter != t);

            // Mark the position of the delimiter so it can be preserved.
            let at_delimiter = parse_current_marker!();

            match input.eat(|t| &delimiter == t) {
                Some(_) => {
                    // The delimiter exists. Put it back when it must survive.
                    if let RecoveryPoint::AtDelimiter = point {
                        input.rewind_to(at_delimiter);
                    }

                    Ok(Recovery::Recovered { skipped, error })
                }
                None => {
                    // EOF was reached without ever finding the delimiter: the
                    // error cannot be recovered from. Report it as such while
                    // retaining the original error's parser and context.
                    let message: crate::error::Expected<I::Token, I::Slice> = format!(
                        "unrecoverable parse error: reached EOF before finding \
                         recovery delimiter; original error: {}",
                        error.error
                    ).into();
                    let constructed: Result<Recovery<O, I>, I> = parse_error!(message);
                    let mut unrecoverable = match constructed {
                        Err(e) => e,
                        // `parse_error!` always constructs an `Err`.
                        Ok(_) => unreachable!(),
                    };

                    unrecoverable.push_info(error.info.parser, error.info.context);
                    for frame in error.stack {
                        unrecoverable.push_info(frame.parser, frame.context);
                    }

                    Err(unrecoverable)
                }
            }
        }
    }
}
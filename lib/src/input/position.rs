use crate::input::{Input, Show};

/// A read-only observation of an input's current parse position.
///
/// Returned by [`Input::position()`]. Observing the position neither clones
/// nor advances the input: it simply reports the current input unit (token)
/// and the number of units consumed so far, in the input's own units.
///
/// [`Input::position()`]: crate::input::Input::position()
pub struct Position<I: Input> {
    /// The current input unit (token), if any remains.
    pub unit: Option<I::Token>,
    /// The number of input units consumed so far, in the input's own units.
    pub consumed: usize,
}

impl<I: Input> Position<I> {
    /// Creates a new position observation from a `unit` and `consumed` count.
    #[inline(always)]
    pub fn new(unit: Option<I::Token>, consumed: usize) -> Position<I> {
        Position { unit, consumed }
    }
}

impl<I: Input> PartialEq for Position<I> {
    fn eq(&self, other: &Position<I>) -> bool {
        self.consumed == other.consumed && self.unit == other.unit
    }
}

impl<I: Input> std::fmt::Debug for Position<I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Position {{ unit: ")?;
        match self.unit {
            Some(ref unit) => write!(f, "Some({})", unit as &dyn Show)?,
            None => write!(f, "None")?,
        }

        write!(f, ", consumed: {:?} }}", self.consumed)
    }
}

impl<I: Input> Show for Position<I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} unit(s) consumed", self.consumed)?;
        if let Some(ref unit) = self.unit {
            write!(f, ", next unit {}", unit as &dyn Show)?;
        }

        Ok(())
    }
}

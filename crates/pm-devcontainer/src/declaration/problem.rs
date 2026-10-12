//! What is wrong with, or unsupported in, a declaration.

/// One field of a declaration that cannot be honoured, and why.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Problem {
    /// The property path in `devcontainer.json`, such as `features` or `mounts[1]`.
    pub field: String,
    /// What is wrong, and what to change.
    pub message: String,
}

impl Problem {
    /// Names `field` and says what is wrong with it.
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Problem {
    /// Writes `field: message`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

/// Every problem found in one pass, so a declaration is fixed once rather than field by field.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Problems(pub Vec<Problem>);

impl Problems {
    /// Records one problem.
    pub fn add(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.0.push(Problem::new(field, message));
    }

    /// Whether nothing was found.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Succeeds with `value` when nothing was found, otherwise fails with everything found.
    pub fn into_result<T>(self, value: T) -> Result<T, Self> {
        if self.is_empty() {
            Ok(value)
        } else {
            Err(self)
        }
    }
}

impl std::fmt::Display for Problems {
    /// Lists the problems one per line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, problem) in self.0.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "{problem}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Problems {}

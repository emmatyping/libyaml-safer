pub type Result<T, E = Error> = core::result::Result<T, E>;

/// The pointer position.
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Mark {
    /// The position index.
    pub index: u64,
    /// The position line.
    pub line: u64,
    /// The position column.
    pub column: u64,
}

impl std::fmt::Display for Mark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {} column {}", self.line + 1, self.column + 1)
    }
}

#[derive(Debug)]
#[allow(clippy::struct_field_names)]
struct Problem {
    pub problem: &'static str,
    pub problem_mark: Mark,
    pub context: &'static str,
    pub context_mark: Mark,
}

#[derive(Debug)]
enum ErrorImpl {
    Reader {
        problem: &'static str,
        offset: usize,
        value: i32,
    },
    Scanner(Problem),
    Parser(Problem),
    Composer(Problem),
    Emitter(&'static str),
    Io(std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Reader,
    Scanner,
    Parser,
    Composer,
    Emitter,
    Io,
}

#[derive(Debug)]
pub struct Error(Box<ErrorImpl>);

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self(Box::new(ErrorImpl::Io(value)))
    }
}

impl Error {
    pub(crate) fn reader(problem: &'static str, offset: usize, value: i32) -> Self {
        Self(Box::new(ErrorImpl::Reader {
            problem,
            offset,
            value,
        }))
    }

    pub(crate) fn scanner(
        context: &'static str,
        context_mark: Mark,
        problem: &'static str,
        problem_mark: Mark,
    ) -> Self {
        Self(Box::new(ErrorImpl::Scanner(Problem {
            problem,
            problem_mark,
            context,
            context_mark,
        })))
    }

    pub(crate) fn parser(
        context: &'static str,
        context_mark: Mark,
        problem: &'static str,
        problem_mark: Mark,
    ) -> Self {
        Self(Box::new(ErrorImpl::Parser(Problem {
            problem,
            problem_mark,
            context,
            context_mark,
        })))
    }

    pub(crate) fn composer(
        context: &'static str,
        context_mark: Mark,
        problem: &'static str,
        problem_mark: Mark,
    ) -> Self {
        Self(Box::new(ErrorImpl::Composer(Problem {
            problem,
            problem_mark,
            context,
            context_mark,
        })))
    }

    pub(crate) fn emitter(problem: &'static str) -> Self {
        Self(Box::new(ErrorImpl::Emitter(problem)))
    }

    pub fn kind(&self) -> ErrorKind {
        match &*self.0 {
            ErrorImpl::Reader { .. } => ErrorKind::Reader,
            ErrorImpl::Scanner(_) => ErrorKind::Scanner,
            ErrorImpl::Parser(_) => ErrorKind::Parser,
            ErrorImpl::Composer(_) => ErrorKind::Composer,
            ErrorImpl::Emitter(_) => ErrorKind::Emitter,
            ErrorImpl::Io(_) => ErrorKind::Io,
        }
    }

    pub fn problem_mark(&self) -> Option<Mark> {
        match &*self.0 {
            ErrorImpl::Reader { .. } | ErrorImpl::Emitter(_) | ErrorImpl::Io(_) => None,
            ErrorImpl::Scanner(p) | ErrorImpl::Parser(p) | ErrorImpl::Composer(p) => {
                Some(p.problem_mark)
            }
        }
    }

    pub fn context_mark(&self) -> Option<Mark> {
        match &*self.0 {
            ErrorImpl::Reader { .. } | ErrorImpl::Emitter(..) | ErrorImpl::Io(_) => None,
            ErrorImpl::Scanner(p) | ErrorImpl::Parser(p) | ErrorImpl::Composer(p) => {
                if p.context.is_empty() {
                    None
                } else {
                    Some(p.context_mark)
                }
            }
        }
    }

    pub fn problem(&self) -> &'static str {
        match &*self.0 {
            ErrorImpl::Reader { problem, .. } | ErrorImpl::Emitter(problem) => problem,
            ErrorImpl::Scanner(p) | ErrorImpl::Parser(p) | ErrorImpl::Composer(p) => p.problem,
            ErrorImpl::Io(_) => "I/O error",
        }
    }

    pub fn context(&self) -> Option<&'static str> {
        match &*self.0 {
            ErrorImpl::Reader { .. } | ErrorImpl::Emitter(..) | ErrorImpl::Io(_) => None,
            ErrorImpl::Scanner(p) | ErrorImpl::Parser(p) | ErrorImpl::Composer(p) => {
                if p.context.is_empty() {
                    None
                } else {
                    Some(p.context)
                }
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let ErrorImpl::Io(err) = &*self.0 {
            Some(err)
        } else {
            None
        }
    }
}

impl TryFrom<Error> for std::io::Error {
    type Error = Error;

    fn try_from(value: Error) -> Result<Self, Self::Error> {
        if value.kind() == ErrorKind::Io {
            if let ErrorImpl::Io(err) = *value.0 {
                Ok(err)
            } else {
                unreachable!()
            }
        } else {
            Err(value)
        }
    }
}

impl core::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ErrorKind::Reader => "Reader",
            ErrorKind::Scanner => "Scanner",
            ErrorKind::Parser => "Parser",
            ErrorKind::Composer => "Composer",
            ErrorKind::Emitter => "Emitter",
            ErrorKind::Io => "I/O",
        })
    }
}

impl core::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            problem,
            problem_mark,
            context,
            context_mark,
        } = self;

        if self.context.is_empty() {
            write!(f, "{problem_mark}: {problem}")
        } else {
            write!(f, "{problem_mark}: {problem} {context} ({context_mark})")
        }
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} error: ", self.kind())?;
        match *self.0 {
            ErrorImpl::Reader {
                problem,
                offset,
                value,
            } => write!(f, "{problem} (offset {offset}, value {value})"),
            ErrorImpl::Scanner(ref p) | ErrorImpl::Parser(ref p) | ErrorImpl::Composer(ref p) => {
                write!(f, "{p}")
            }
            ErrorImpl::Emitter(problem) => write!(f, "{problem}"),
            ErrorImpl::Io(ref err) => write!(f, "{err}"),
        }
    }
}

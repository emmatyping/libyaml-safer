#![doc = include_str!("../README.md")]
#![doc(html_root_url = "https://docs.rs/libyaml-safer/0.1.0")]
#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::fn_params_excessive_bools,
    clippy::manual_range_contains,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::match_wildcard_for_single_variants
)]
#![deny(unsafe_code)]

extern crate alloc;

#[macro_use]
mod macros;

mod document;
mod emitter;
mod error;
mod event;
mod parser;
mod reader;
mod scanner;
mod token;

pub use crate::document::*;
pub use crate::emitter::*;
pub use crate::error::*;
pub use crate::event::*;
pub use crate::parser::*;
pub use crate::scanner::*;
pub use crate::token::*;

pub(crate) const INPUT_RAW_BUFFER_SIZE: usize = 16384;
pub(crate) const INPUT_BUFFER_SIZE: usize = INPUT_RAW_BUFFER_SIZE;
pub(crate) const OUTPUT_BUFFER_SIZE: usize = 16384;

/// The tag `!!null` with the only possible value: `null`.
pub const NULL_TAG: &str = "tag:yaml.org,2002:null";
/// The tag `!!bool` with the values: `true` and `false`.
pub const BOOL_TAG: &str = "tag:yaml.org,2002:bool";
/// The tag `!!str` for string values.
pub const STR_TAG: &str = "tag:yaml.org,2002:str";
/// The tag `!!int` for integer values.
pub const INT_TAG: &str = "tag:yaml.org,2002:int";
/// The tag `!!float` for float values.
pub const FLOAT_TAG: &str = "tag:yaml.org,2002:float";
/// The tag `!!timestamp` for date and time values.
pub const TIMESTAMP_TAG: &str = "tag:yaml.org,2002:timestamp";

/// The tag `!!seq` is used to denote sequences.
pub const SEQ_TAG: &str = "tag:yaml.org,2002:seq";
/// The tag `!!map` is used to denote mapping.
pub const MAP_TAG: &str = "tag:yaml.org,2002:map";

/// The default scalar tag is `!!str`.
pub const DEFAULT_SCALAR_TAG: &str = STR_TAG;
/// The default sequence tag is `!!seq`.
pub const DEFAULT_SEQUENCE_TAG: &str = SEQ_TAG;
/// The default mapping tag is `!!map`.
pub const DEFAULT_MAPPING_TAG: &str = MAP_TAG;

/// The version directive data.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct VersionDirective {
    /// The major version number.
    pub major: i32,
    /// The minor version number.
    pub minor: i32,
}

/// The tag directive data.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct TagDirective {
    /// The tag handle.
    pub handle: String,
    /// The tag prefix.
    pub prefix: String,
}

/// The stream encoding.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum Encoding {
    /// Let the parser choose the encoding.
    #[default]
    Any = 0,
    /// The default UTF-8 encoding.
    Utf8 = 1,
    /// The UTF-16-LE encoding with BOM.
    Utf16Le = 2,
    /// The UTF-16-BE encoding with BOM.
    Utf16Be = 3,
}

/// Line break type.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum Break {
    /// Let the parser choose the break type.
    #[default]
    Any = 0,
    /// Use CR for line breaks (Mac style).
    Cr = 1,
    /// Use LN for line breaks (Unix style).
    Ln = 2,
    /// Use CR LN for line breaks (DOS style).
    CrLn = 3,
}

/// Scalar styles.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum ScalarStyle {
    /// Let the emitter choose the style.
    #[default]
    Any = 0,
    /// The plain scalar style.
    Plain = 1,
    /// The single-quoted scalar style.
    SingleQuoted = 2,
    /// The double-quoted scalar style.
    DoubleQuoted = 3,
    /// The literal scalar style.
    Literal = 4,
    /// The folded scalar style.
    Folded = 5,
}

/// Sequence styles.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum SequenceStyle {
    /// Let the emitter choose the style.
    Any = 0,
    /// The block sequence style.
    Block = 1,
    /// The flow sequence style.
    Flow = 2,
}

/// Mapping styles.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum MappingStyle {
    /// Let the emitter choose the style.
    Any = 0,
    /// The block mapping style.
    Block = 1,
    /// The flow mapping style.
    Flow = 2,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanity() {
        const SANITY_INPUT: &str = r#"unicode: "Sosa did fine.\u263A"
control: "\b1998\t1999\t2000\n"
hex esc: "\x0d\x0a is \r\n"

single: '"Howdy!" he cried.'
quoted: ' # Not a ''comment''.'
tie-fighter: '|\-*-/|'
"#;
        const SANITY_OUTPUT: &str = r#"unicode: "Sosa did fine.\u263A"
control: "\b1998\t1999\t2000\n"
hex esc: "\r\n is \r\n"
single: '"Howdy!" he cried.'
quoted: ' # Not a ''comment''.'
tie-fighter: '|\-*-/|'
"#;
        let mut parser = Parser::new();
        let mut read_in = SANITY_INPUT.as_bytes();
        parser.set_input_string(&mut read_in);
        let doc = Document::load(&mut parser).unwrap();

        let mut emitter = Emitter::new();
        let mut output = Vec::new();
        emitter.set_output(&mut output);
        doc.dump(&mut emitter).unwrap();
        let output_str = core::str::from_utf8(&output).expect("invalid UTF-8");
        assert_eq!(output_str, SANITY_OUTPUT);
    }

    #[test]
    fn scanner_marks() {
        const INPUT: &str = "b:
c: true";
        let mut scanner = Scanner::new();
        let mut read_in = INPUT.as_bytes();
        scanner.set_input(&mut read_in);
        let events = scanner.collect::<Result<Vec<_>, _>>().unwrap();
        let expected = &[
            Token {
                data: TokenData::StreamStart {
                    encoding: Encoding::Utf8,
                },
                start_mark: Mark {
                    index: 0,
                    line: 0,
                    column: 0,
                },
                end_mark: Mark {
                    index: 0,
                    line: 0,
                    column: 0,
                },
            },
            Token {
                data: TokenData::BlockMappingStart,
                start_mark: Mark {
                    index: 0,
                    line: 0,
                    column: 0,
                },
                end_mark: Mark {
                    index: 0,
                    line: 0,
                    column: 0,
                },
            },
            Token {
                data: TokenData::Key,
                start_mark: Mark {
                    index: 0,
                    line: 0,
                    column: 0,
                },
                end_mark: Mark {
                    index: 0,
                    line: 0,
                    column: 0,
                },
            },
            Token {
                data: TokenData::Scalar {
                    value: String::from("b"),
                    style: ScalarStyle::Plain,
                },
                start_mark: Mark {
                    index: 0,
                    line: 0,
                    column: 0,
                },
                end_mark: Mark {
                    index: 1,
                    line: 0,
                    column: 1,
                },
            },
            Token {
                data: TokenData::Value,
                start_mark: Mark {
                    index: 1,
                    line: 0,
                    column: 1,
                },
                end_mark: Mark {
                    index: 2,
                    line: 0,
                    column: 2,
                },
            },
            Token {
                data: TokenData::Key,
                start_mark: Mark {
                    index: 3,
                    line: 1,
                    column: 0,
                },
                end_mark: Mark {
                    index: 3,
                    line: 1,
                    column: 0,
                },
            },
            Token {
                data: TokenData::Scalar {
                    value: String::from("c"),
                    style: ScalarStyle::Plain,
                },
                start_mark: Mark {
                    index: 3,
                    line: 1,
                    column: 0,
                },
                end_mark: Mark {
                    index: 4,
                    line: 1,
                    column: 1,
                },
            },
            Token {
                data: TokenData::Value,
                start_mark: Mark {
                    index: 4,
                    line: 1,
                    column: 1,
                },
                end_mark: Mark {
                    index: 5,
                    line: 1,
                    column: 2,
                },
            },
            Token {
                data: TokenData::Scalar {
                    value: String::from("true"),
                    style: ScalarStyle::Plain,
                },
                start_mark: Mark {
                    index: 6,
                    line: 1,
                    column: 3,
                },
                end_mark: Mark {
                    index: 10,
                    line: 1,
                    column: 7,
                },
            },
            Token {
                data: TokenData::BlockEnd,
                start_mark: Mark {
                    index: 10,
                    line: 2,
                    column: 0,
                },
                end_mark: Mark {
                    index: 10,
                    line: 2,
                    column: 0,
                },
            },
            Token {
                data: TokenData::StreamEnd,
                start_mark: Mark {
                    index: 10,
                    line: 2,
                    column: 0,
                },
                end_mark: Mark {
                    index: 10,
                    line: 2,
                    column: 0,
                },
            },
        ];
        assert_eq!(
            events,
            expected,
            "diff:\n{}",
            zip_longest(
                format!("{events:#?}").lines(),
                format!("{expected:#?}").lines()
            )
            .map(|(a, b)| {
                let a = a.unwrap_or_default();
                let b = b.unwrap_or_default();
                format!("{a:<40} {b}")
            })
            .collect::<Vec<_>>()
            .join("\n")
        );
    }

    fn zip_longest<A: Iterator, B: Iterator>(
        a: A,
        b: B,
    ) -> impl Iterator<Item = (Option<A::Item>, Option<B::Item>)> {
        let mut a = a.map(Some).collect::<Vec<_>>();
        let mut b = b.map(Some).collect::<Vec<_>>();
        let len = a.len().max(b.len());
        a.resize_with(len, || None);
        b.resize_with(len, || None);
        a.into_iter()
            .zip(b)
            .take_while(|(a, b)| a.is_some() || b.is_some())
    }
}

#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::items_after_statements,
    clippy::let_underscore_untyped,
    clippy::missing_errors_doc,
    clippy::missing_safety_doc,
    clippy::too_many_lines
)]

use libyaml_safer::{EventData, Parser, ScalarStyle};
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{self, Read, Write};
use std::process::{self, ExitCode};
use std::slice;

pub(crate) fn test_main(
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
) -> Result<(), Box<dyn Error>> {
    let mut parser = Parser::new();

    let mut stdin = std::io::BufReader::new(stdin);
    parser.set_input(&mut stdin);

    loop {
        let event = match parser.parse() {
            Err(err) => {
                let error = format!("Parse error: {err}");
                return Err(error.into());
            }
            Ok(event) => event,
        };

        let mut is_end = false;

        match &event.data {
            EventData::StreamStart { .. } => {
                _ = writeln!(stdout, "+STR");
            }
            EventData::StreamEnd => {
                is_end = true;
                _ = writeln!(stdout, "-STR");
            }
            EventData::DocumentStart { implicit, .. } => {
                _ = write!(stdout, "+DOC");
                if !implicit {
                    _ = write!(stdout, " ---");
                }
                _ = writeln!(stdout);
            }
            EventData::DocumentEnd { implicit } => {
                _ = write!(stdout, "-DOC");
                if !implicit {
                    _ = write!(stdout, " ...");
                }
                _ = writeln!(stdout);
            }
            EventData::Alias { anchor } => {
                _ = writeln!(stdout, "=ALI *{anchor}");
            }
            EventData::Scalar {
                anchor,
                tag,
                value,
                style,
                ..
            } => {
                let _ = write!(stdout, "=VAL");
                if let Some(anchor) = anchor {
                    _ = write!(stdout, " &{anchor}");
                }
                if let Some(tag) = tag {
                    _ = write!(stdout, " <{tag}>");
                }
                _ = stdout.write_all(match style {
                    ScalarStyle::Plain => b" :",
                    ScalarStyle::SingleQuoted => b" '",
                    ScalarStyle::DoubleQuoted => b" \"",
                    ScalarStyle::Literal => b" |",
                    ScalarStyle::Folded => b" >",
                    _ => process::abort(),
                });
                print_escaped(stdout, value);
                _ = writeln!(stdout);
            }
            EventData::SequenceStart { anchor, tag, .. } => {
                let _ = write!(stdout, "+SEQ");
                if let Some(anchor) = anchor {
                    _ = write!(stdout, " &{anchor}");
                }
                if let Some(tag) = tag {
                    _ = write!(stdout, " <{tag}>");
                }
                _ = writeln!(stdout);
            }
            EventData::SequenceEnd => {
                _ = writeln!(stdout, "-SEQ");
            }
            EventData::MappingStart { anchor, tag, .. } => {
                let _ = write!(stdout, "+MAP");
                if let Some(anchor) = anchor {
                    _ = write!(stdout, " &{anchor}");
                }
                if let Some(tag) = tag {
                    _ = write!(stdout, " <{tag}>");
                }
                _ = writeln!(stdout);
            }
            EventData::MappingEnd => {
                _ = writeln!(stdout, "-MAP");
            }
        }

        if is_end {
            break;
        }
    }
    Ok(())
}

fn print_escaped(stdout: &mut dyn Write, s: &str) {
    for ch in s.bytes() {
        let repr = match &ch {
            b'\\' => b"\\\\",
            b'\0' => b"\\0",
            b'\x08' => b"\\b",
            b'\n' => b"\\n",
            b'\r' => b"\\r",
            b'\t' => b"\\t",
            c => slice::from_ref(c),
        };
        let _ = stdout.write_all(repr);
    }
}

fn main() -> ExitCode {
    let args = env::args_os().skip(1);
    if args.len() == 0 {
        let _ = writeln!(io::stderr(), "Usage: run-parser-test-suite <in.yaml>...");
        return ExitCode::FAILURE;
    }
    for arg in args {
        let mut stdin = File::open(arg).unwrap();
        let mut stdout = io::stdout();
        let result = test_main(&mut stdin, &mut stdout);
        if let Err(err) = result {
            let _ = writeln!(io::stderr(), "{err}");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

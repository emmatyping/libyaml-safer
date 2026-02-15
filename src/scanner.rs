use std::collections::VecDeque;

use crate::char_traits::{is_blank, is_blank_or_breakz, is_break, is_hex, as_hex};
use crate::input::Input;
use crate::{Encoding, Error, Mark, Result, ScalarStyle, SimpleKey, Token, TokenData};

const MAX_NUMBER_LENGTH: u64 = 9_u64;

/// Given an input stream of characters, produce a stream of [`Token`]s.
///
/// This is used internally by the parser, and may also be used standalone as a
/// replacement for the libyaml `yaml_parser_scan()` function.
pub struct Scanner<I> {
    /// The input source.
    pub(crate) input: I,
    /// The mark of the current position.
    pub(crate) mark: Mark,
    /// Have we started to scan the input stream?
    pub(crate) stream_start_produced: bool,
    /// Have we reached the end of the input stream?
    pub(crate) stream_end_produced: bool,
    /// The number of unclosed '[' and '{' indicators.
    pub(crate) flow_level: i32,
    /// The tokens queue.
    pub(crate) tokens: VecDeque<Token>,
    /// The number of tokens fetched from the queue.
    pub(crate) tokens_parsed: usize,
    /// Does the tokens queue contain a token ready for dequeueing.
    pub(crate) token_available: bool,
    /// The indentation levels stack.
    pub(crate) indents: Vec<i32>,
    /// The current indentation level.
    pub(crate) indent: i32,
    /// May a simple key occur at the current position?
    pub(crate) simple_key_allowed: bool,
    /// The stack of simple keys.
    pub(crate) simple_keys: Vec<SimpleKey>,
}

impl<I: Input> Scanner<I> {
    /// Create a new scanner for the given input.
    pub fn new(input: I) -> Scanner<I> {
        Self {
            input,
            mark: Mark::default(),
            stream_start_produced: false,
            stream_end_produced: false,
            flow_level: 0,
            tokens: VecDeque::with_capacity(16),
            tokens_parsed: 0,
            token_available: false,
            indents: Vec::with_capacity(16),
            indent: 0,
            simple_key_allowed: false,
            simple_keys: Vec::with_capacity(16),
        }
    }

    /// Reset the scanner with a new input, preserving internal allocations.
    pub fn reset(&mut self, input: I) {
        self.input = input;
        self.mark = Mark::default();
        self.stream_start_produced = false;
        self.stream_end_produced = false;
        self.flow_level = 0;
        self.tokens.clear();
        self.tokens_parsed = 0;
        self.token_available = false;
        self.indents.clear();
        self.indent = 0;
        self.simple_key_allowed = false;
        self.simple_keys.clear();
    }

    // -----------------------------------------------------------------------
    // Low-level input helpers
    // -----------------------------------------------------------------------

    /// Skip one character, advancing the mark.
    #[inline]
    fn skip_char(&mut self) {
        let ch = self.input.peek();
        self.input.skip();
        self.mark.index += ch.len_utf8() as u64;
        self.mark.column += 1;
    }

    /// Read one character into `string`, advancing the mark.
    #[inline]
    fn read_char(&mut self, string: &mut String) {
        let ch = self.input.peek();
        string.push(ch);
        self.input.skip();
        self.mark.index += ch.len_utf8() as u64;
        self.mark.column += 1;
    }

    /// Skip a line break sequence (\r\n, \r, \n, or Unicode break), advancing the mark.
    fn skip_line_break(&mut self) {
        let front = self.input.peek();
        if front == '\r' && self.input.peek_nth(1) == '\n' {
            self.input.skip_n(2);
            self.mark.index += 2;
            self.mark.column = 0;
            self.mark.line += 1;
        } else if is_break(front) {
            let width = front.len_utf8();
            self.input.skip();
            self.mark.index += width as u64;
            self.mark.column = 0;
            self.mark.line += 1;
        }
    }

    /// Read a line break sequence into `string`, advancing the mark.
    fn read_line_break(&mut self, string: &mut String) {
        let front = self.input.peek();
        if front == '\r' && self.input.peek_nth(1) == '\n' {
            string.push('\n');
            self.input.skip_n(2);
            self.mark.index += 2;
            self.mark.column = 0;
            self.mark.line += 1;
        } else if is_break(front) {
            self.input.skip();
            let char_len = front.len_utf8();
            if char_len == 3 {
                // libyaml preserves Unicode breaks (LS, PS) as-is.
                string.push(front);
            } else {
                string.push('\n');
            }
            self.mark.index += char_len as u64;
            self.mark.column = 0;
            self.mark.line += 1;
        }
    }

    // -----------------------------------------------------------------------
    // Public scanning API
    // -----------------------------------------------------------------------

    /// Scan the input stream and produce the next token.
    ///
    /// Call the function subsequently to produce a sequence of tokens
    /// corresponding to the input stream. The initial token has the type
    /// [`TokenData::StreamStart`] while the ending token has the type
    /// [`TokenData::StreamEnd`].
    pub fn scan(&mut self) -> Result<Token> {
        if self.stream_end_produced {
            return Ok(Token {
                data: TokenData::StreamEnd,
                start_mark: Mark::default(),
                end_mark: Mark::default(),
            });
        }
        if !self.token_available {
            self.fetch_more_tokens()?;
        }
        if let Some(token) = self.tokens.pop_front() {
            self.token_available = false;
            self.tokens_parsed += 1;
            if let TokenData::StreamEnd = &token.data {
                self.stream_end_produced = true;
            }
            Ok(token)
        } else {
            unreachable!("no more tokens, but stream-end was not produced")
        }
    }

    /// Equivalent of the libyaml `PEEK_TOKEN` macro, used by the parser.
    pub(crate) fn peek(&mut self) -> Result<&Token> {
        if self.token_available {
            return Ok(self
                .tokens
                .front()
                .expect("token_available is true, but token queue is empty"));
        }
        self.fetch_more_tokens()?;
        assert!(
            self.token_available,
            "fetch_more_tokens() did not produce any tokens, nor an error"
        );
        Ok(self
            .tokens
            .front()
            .expect("token_available is true, but token queue is empty"))
    }

    /// Equivalent of the libyaml `PEEK_TOKEN` macro, used by the parser.
    pub(crate) fn peek_mut(&mut self) -> Result<&mut Token> {
        if self.token_available {
            return Ok(self
                .tokens
                .front_mut()
                .expect("token_available is true, but token queue is empty"));
        }
        self.fetch_more_tokens()?;
        assert!(
            self.token_available,
            "fetch_more_tokens() did not produce any tokens, nor an error"
        );
        Ok(self
            .tokens
            .front_mut()
            .expect("token_available is true, but token queue is empty"))
    }

    /// Equivalent of the libyaml `SKIP_TOKEN` macro, used by the parser.
    pub(crate) fn skip_token(&mut self) {
        self.token_available = false;
        self.tokens_parsed = self.tokens_parsed.wrapping_add(1);
        let skipped = self.tokens.pop_front().expect("SKIP_TOKEN but EOF");
        self.stream_end_produced = matches!(
            skipped,
            Token {
                data: TokenData::StreamEnd,
                ..
            }
        );
    }

    fn set_scanner_error<T>(
        &mut self,
        context: &'static str,
        context_mark: Mark,
        problem: &'static str,
    ) -> Result<T> {
        Err(Error::scanner(context, context_mark, problem, self.mark))
    }

    // -----------------------------------------------------------------------
    // Token fetching
    // -----------------------------------------------------------------------

    pub(crate) fn fetch_more_tokens(&mut self) -> Result<()> {
        let mut need_more_tokens;
        loop {
            need_more_tokens = false;
            if self.tokens.is_empty() {
                need_more_tokens = true;
            } else {
                self.stale_simple_keys()?;
                for simple_key in &self.simple_keys {
                    if simple_key.possible && simple_key.token_number == self.tokens_parsed {
                        need_more_tokens = true;
                        break;
                    }
                }
            }
            if !need_more_tokens {
                break;
            }
            self.fetch_next_token()?;
        }
        self.token_available = true;
        Ok(())
    }

    fn fetch_next_token(&mut self) -> Result<()> {
        self.input.lookahead(1);
        if !self.stream_start_produced {
            self.fetch_stream_start();
            return Ok(());
        }
        self.scan_to_next_token()?;
        self.stale_simple_keys()?;
        self.unroll_indent(self.mark.column as i64);
        self.input.lookahead(4);
        if self.input.next_is_z() {
            return self.fetch_stream_end();
        }
        if self.mark.column == 0_u64 && self.input.next_char_is('%') {
            return self.fetch_directive();
        }
        if self.mark.column == 0_u64
            && self.input.nth_char_is(0, '-')
            && self.input.nth_char_is(1, '-')
            && self.input.nth_char_is(2, '-')
            && is_blank_or_breakz(self.input.peek_nth(3))
        {
            return self.fetch_document_indicator(TokenData::DocumentStart);
        }
        if self.mark.column == 0_u64
            && self.input.nth_char_is(0, '.')
            && self.input.nth_char_is(1, '.')
            && self.input.nth_char_is(2, '.')
            && is_blank_or_breakz(self.input.peek_nth(3))
        {
            return self.fetch_document_indicator(TokenData::DocumentEnd);
        }
        if self.input.next_char_is('[') {
            return self.fetch_flow_collection_start(TokenData::FlowSequenceStart);
        }
        if self.input.next_char_is('{') {
            return self.fetch_flow_collection_start(TokenData::FlowMappingStart);
        }
        if self.input.next_char_is(']') {
            return self.fetch_flow_collection_end(TokenData::FlowSequenceEnd);
        }
        if self.input.next_char_is('}') {
            return self.fetch_flow_collection_end(TokenData::FlowMappingEnd);
        }
        if self.input.next_char_is(',') {
            return self.fetch_flow_entry();
        }
        if self.input.next_char_is('-') && is_blank_or_breakz(self.input.peek_nth(1)) {
            return self.fetch_block_entry();
        }
        if self.input.next_char_is('?')
            && (self.flow_level != 0 || is_blank_or_breakz(self.input.peek_nth(1)))
        {
            return self.fetch_key();
        }
        if self.input.next_char_is(':')
            && (self.flow_level != 0 || is_blank_or_breakz(self.input.peek_nth(1)))
        {
            return self.fetch_value();
        }
        if self.input.next_char_is('*') {
            return self.fetch_anchor(true);
        }
        if self.input.next_char_is('&') {
            return self.fetch_anchor(false);
        }
        if self.input.next_char_is('!') {
            return self.fetch_tag();
        }
        if self.input.next_char_is('|') && self.flow_level == 0 {
            return self.fetch_block_scalar(true);
        }
        if self.input.next_char_is('>') && self.flow_level == 0 {
            return self.fetch_block_scalar(false);
        }
        if self.input.next_char_is('\'') {
            return self.fetch_flow_scalar(true);
        }
        if self.input.next_char_is('"') {
            return self.fetch_flow_scalar(false);
        }
        if !(self.input.next_is_blank_or_breakz()
            || self.input.next_char_is('-')
            || self.input.next_char_is('?')
            || self.input.next_char_is(':')
            || self.input.next_char_is(',')
            || self.input.next_char_is('[')
            || self.input.next_char_is(']')
            || self.input.next_char_is('{')
            || self.input.next_char_is('}')
            || self.input.next_char_is('#')
            || self.input.next_char_is('&')
            || self.input.next_char_is('*')
            || self.input.next_char_is('!')
            || self.input.next_char_is('|')
            || self.input.next_char_is('>')
            || self.input.next_char_is('\'')
            || self.input.next_char_is('"')
            || self.input.next_char_is('%')
            || self.input.next_char_is('@')
            || self.input.next_char_is('`'))
            || self.input.next_char_is('-') && !is_blank(self.input.peek_nth(1))
            || self.flow_level == 0
                && (self.input.next_char_is('?') || self.input.next_char_is(':'))
                && !is_blank_or_breakz(self.input.peek_nth(1))
        {
            return self.fetch_plain_scalar();
        }
        self.set_scanner_error(
            "while scanning for the next token",
            self.mark,
            "found character that cannot start any token",
        )
    }

    // -----------------------------------------------------------------------
    // Simple key handling
    // -----------------------------------------------------------------------

    fn stale_simple_keys(&mut self) -> Result<()> {
        for simple_key in &mut self.simple_keys {
            let mark = simple_key.mark;
            if simple_key.possible
                && (mark.line < self.mark.line || mark.index + 1024 < self.mark.index)
            {
                if simple_key.required {
                    return self.set_scanner_error(
                        "while scanning a simple key",
                        mark,
                        "could not find expected ':'",
                    );
                }
                simple_key.possible = false;
            }
        }

        Ok(())
    }

    fn save_simple_key(&mut self) -> Result<()> {
        let required = self.flow_level == 0 && self.indent as u64 == self.mark.column;
        if self.simple_key_allowed {
            let simple_key = SimpleKey {
                possible: true,
                required,
                token_number: self.tokens_parsed + self.tokens.len(),
                mark: self.mark,
            };
            self.remove_simple_key()?;
            *self.simple_keys.last_mut().unwrap() = simple_key;
        }
        Ok(())
    }

    fn remove_simple_key(&mut self) -> Result<()> {
        let simple_key: &mut SimpleKey = self.simple_keys.last_mut().unwrap();
        if simple_key.possible {
            let mark = simple_key.mark;
            if simple_key.required {
                return self.set_scanner_error(
                    "while scanning a simple key",
                    mark,
                    "could not find expected ':'",
                );
            }
        }
        simple_key.possible = false;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Flow level / indentation
    // -----------------------------------------------------------------------

    fn increase_flow_level(&mut self) -> Result<()> {
        let empty_simple_key = SimpleKey {
            possible: false,
            required: false,
            token_number: 0,
            mark: Mark {
                index: 0_u64,
                line: 0_u64,
                column: 0_u64,
            },
        };
        self.simple_keys.push(empty_simple_key);
        assert!(
            self.flow_level != i32::MAX,
            "parser.flow_level integer overflow"
        );
        self.flow_level += 1;
        Ok(())
    }

    fn decrease_flow_level(&mut self) {
        if self.flow_level != 0 {
            self.flow_level -= 1;
            let _ = self.simple_keys.pop();
        }
    }

    fn roll_indent(
        &mut self,
        column: i64,
        number: i64,
        data: TokenData,
        mark: Mark,
    ) -> Result<()> {
        if self.flow_level != 0 {
            return Ok(());
        }
        if self.indent < column as i32 {
            self.indents.push(self.indent);
            assert!(column <= i32::MAX as i64, "integer overflow");
            self.indent = column as i32;
            let token = Token {
                data,
                start_mark: mark,
                end_mark: mark,
            };
            if number == -1_i64 {
                self.tokens.push_back(token);
            } else {
                self.tokens
                    .insert((number as usize).wrapping_sub(self.tokens_parsed), token);
            }
        }
        Ok(())
    }

    fn unroll_indent(&mut self, column: i64) {
        if self.flow_level != 0 {
            return;
        }
        while self.indent as i64 > column {
            let token = Token {
                data: TokenData::BlockEnd,
                start_mark: self.mark,
                end_mark: self.mark,
            };
            self.tokens.push_back(token);
            self.indent = self.indents.pop().unwrap();
        }
    }

    // -----------------------------------------------------------------------
    // Fetch helpers
    // -----------------------------------------------------------------------

    fn fetch_stream_start(&mut self) {
        let simple_key = SimpleKey {
            possible: false,
            required: false,
            token_number: 0,
            mark: Mark {
                index: 0,
                line: 0,
                column: 0,
            },
        };
        self.indent = -1;
        self.simple_keys.push(simple_key);
        self.simple_key_allowed = true;
        self.stream_start_produced = true;
        let token = Token {
            data: TokenData::StreamStart {
                encoding: Encoding::Utf8,
            },
            start_mark: self.mark,
            end_mark: self.mark,
        };
        self.tokens.push_back(token);
    }

    fn fetch_stream_end(&mut self) -> Result<()> {
        if self.mark.column != 0_u64 {
            self.mark.column = 0_u64;
            self.mark.line += 1;
        }
        self.unroll_indent(-1_i64);
        self.remove_simple_key()?;
        self.simple_key_allowed = false;
        let token = Token {
            data: TokenData::StreamEnd,
            start_mark: self.mark,
            end_mark: self.mark,
        };
        self.tokens.push_back(token);
        Ok(())
    }

    fn fetch_directive(&mut self) -> Result<()> {
        self.unroll_indent(-1_i64);
        self.remove_simple_key()?;
        self.simple_key_allowed = false;
        let token = self.scan_directive()?;
        self.tokens.push_back(token);
        Ok(())
    }

    fn fetch_document_indicator(&mut self, data: TokenData) -> Result<()> {
        self.unroll_indent(-1_i64);
        self.remove_simple_key()?;
        self.simple_key_allowed = false;
        let start_mark: Mark = self.mark;
        self.skip_char();
        self.skip_char();
        self.skip_char();
        let end_mark: Mark = self.mark;

        let token = Token {
            data,
            start_mark,
            end_mark,
        };
        self.tokens.push_back(token);
        Ok(())
    }

    fn fetch_flow_collection_start(&mut self, data: TokenData) -> Result<()> {
        self.save_simple_key()?;
        self.increase_flow_level()?;
        self.simple_key_allowed = true;
        let start_mark: Mark = self.mark;
        self.skip_char();
        let end_mark: Mark = self.mark;
        let token = Token {
            data,
            start_mark,
            end_mark,
        };
        self.tokens.push_back(token);
        Ok(())
    }

    fn fetch_flow_collection_end(&mut self, data: TokenData) -> Result<()> {
        self.remove_simple_key()?;
        self.decrease_flow_level();
        self.simple_key_allowed = false;
        let start_mark: Mark = self.mark;
        self.skip_char();
        let end_mark: Mark = self.mark;
        let token = Token {
            data,
            start_mark,
            end_mark,
        };
        self.tokens.push_back(token);
        Ok(())
    }

    fn fetch_flow_entry(&mut self) -> Result<()> {
        self.remove_simple_key()?;
        self.simple_key_allowed = true;
        let start_mark: Mark = self.mark;
        self.skip_char();
        let end_mark: Mark = self.mark;
        let token = Token {
            data: TokenData::FlowEntry,
            start_mark,
            end_mark,
        };
        self.tokens.push_back(token);
        Ok(())
    }

    fn fetch_block_entry(&mut self) -> Result<()> {
        if self.flow_level == 0 {
            if !self.simple_key_allowed {
                return self.set_scanner_error(
                    "",
                    self.mark,
                    "block sequence entries are not allowed in this context",
                );
            }
            self.roll_indent(
                self.mark.column as _,
                -1_i64,
                TokenData::BlockSequenceStart,
                self.mark,
            )?;
        }
        self.remove_simple_key()?;
        self.simple_key_allowed = true;
        let start_mark: Mark = self.mark;
        self.skip_char();
        let end_mark: Mark = self.mark;
        let token = Token {
            data: TokenData::BlockEntry,
            start_mark,
            end_mark,
        };
        self.tokens.push_back(token);
        Ok(())
    }

    fn fetch_key(&mut self) -> Result<()> {
        if self.flow_level == 0 {
            if !self.simple_key_allowed {
                return self.set_scanner_error(
                    "",
                    self.mark,
                    "mapping keys are not allowed in this context",
                );
            }
            self.roll_indent(
                self.mark.column as _,
                -1_i64,
                TokenData::BlockMappingStart,
                self.mark,
            )?;
        }
        self.remove_simple_key()?;
        self.simple_key_allowed = self.flow_level == 0;
        let start_mark: Mark = self.mark;
        self.skip_char();
        let end_mark: Mark = self.mark;
        let token = Token {
            data: TokenData::Key,
            start_mark,
            end_mark,
        };
        self.tokens.push_back(token);
        Ok(())
    }

    fn fetch_value(&mut self) -> Result<()> {
        let simple_key: &mut SimpleKey = self.simple_keys.last_mut().unwrap();
        if simple_key.possible {
            let token = Token {
                data: TokenData::Key,
                start_mark: simple_key.mark,
                end_mark: simple_key.mark,
            };
            self.tokens.insert(
                simple_key.token_number.wrapping_sub(self.tokens_parsed),
                token,
            );
            let mark_column = simple_key.mark.column as _;
            let token_number = simple_key.token_number as _;
            let mark = simple_key.mark;
            simple_key.possible = false;
            self.roll_indent(
                mark_column,
                token_number,
                TokenData::BlockMappingStart,
                mark,
            )?;
            self.simple_key_allowed = false;
        } else {
            if self.flow_level == 0 {
                if !self.simple_key_allowed {
                    return self.set_scanner_error(
                        "",
                        self.mark,
                        "mapping values are not allowed in this context",
                    );
                }
                self.roll_indent(
                    self.mark.column as _,
                    -1_i64,
                    TokenData::BlockMappingStart,
                    self.mark,
                )?;
            }
            self.simple_key_allowed = self.flow_level == 0;
        }
        let start_mark: Mark = self.mark;
        self.skip_char();
        let end_mark: Mark = self.mark;
        let token = Token {
            data: TokenData::Value,
            start_mark,
            end_mark,
        };
        self.tokens.push_back(token);
        Ok(())
    }

    fn fetch_anchor(&mut self, fetch_alias_instead_of_anchor: bool) -> Result<()> {
        self.save_simple_key()?;
        self.simple_key_allowed = false;
        let token = self.scan_anchor(fetch_alias_instead_of_anchor)?;
        self.tokens.push_back(token);
        Ok(())
    }

    fn fetch_tag(&mut self) -> Result<()> {
        self.save_simple_key()?;
        self.simple_key_allowed = false;
        let token = self.scan_tag()?;
        self.tokens.push_back(token);
        Ok(())
    }

    fn fetch_block_scalar(&mut self, literal: bool) -> Result<()> {
        self.remove_simple_key()?;
        self.simple_key_allowed = true;
        let token = self.scan_block_scalar(literal)?;
        self.tokens.push_back(token);
        Ok(())
    }

    fn fetch_flow_scalar(&mut self, single: bool) -> Result<()> {
        self.save_simple_key()?;
        self.simple_key_allowed = false;
        let token = self.scan_flow_scalar(single)?;
        self.tokens.push_back(token);
        Ok(())
    }

    fn fetch_plain_scalar(&mut self) -> Result<()> {
        self.save_simple_key()?;
        self.simple_key_allowed = false;
        let token = self.scan_plain_scalar()?;
        self.tokens.push_back(token);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Scanning
    // -----------------------------------------------------------------------

    fn scan_to_next_token(&mut self) -> Result<()> {
        loop {
            self.input.lookahead(1);
            if self.mark.column == 0 && self.input.next_is_bom() {
                self.skip_char();
            }
            self.input.lookahead(1);
            while self.input.next_char_is(' ')
                || (self.flow_level != 0 || !self.simple_key_allowed)
                    && self.input.next_char_is('\t')
            {
                self.skip_char();
                self.input.lookahead(1);
            }
            if self.input.next_char_is('#') {
                while !self.input.next_is_breakz() {
                    self.skip_char();
                    self.input.lookahead(1);
                }
            }
            if !self.input.next_is_break() {
                break;
            }
            self.input.lookahead(2);
            self.skip_line_break();
            if self.flow_level == 0 {
                self.simple_key_allowed = true;
            }
        }
        Ok(())
    }

    fn scan_directive(&mut self) -> Result<Token> {
        let end_mark: Mark;
        let mut major: i32 = 0;
        let mut minor: i32 = 0;
        let start_mark: Mark = self.mark;
        self.skip_char();
        let name = self.scan_directive_name(start_mark)?;
        let token = if name == "YAML" {
            self.scan_version_directive_value(start_mark, &mut major, &mut minor)?;

            end_mark = self.mark;
            Token {
                data: TokenData::VersionDirective { major, minor },
                start_mark,
                end_mark,
            }
        } else if name == "TAG" {
            let (handle, prefix) = self.scan_tag_directive_value(start_mark)?;
            end_mark = self.mark;
            Token {
                data: TokenData::TagDirective { handle, prefix },
                start_mark,
                end_mark,
            }
        } else {
            return self.set_scanner_error(
                "while scanning a directive",
                start_mark,
                "found unknown directive name",
            );
        };
        self.input.lookahead(1);
        loop {
            if !self.input.next_is_blank() {
                break;
            }
            self.skip_char();
            self.input.lookahead(1);
        }

        if self.input.next_char_is('#') {
            loop {
                if self.input.next_is_breakz() {
                    break;
                }
                self.skip_char();
                self.input.lookahead(1);
            }
        }

        if self.input.next_is_breakz() {
            if self.input.next_is_break() {
                self.input.lookahead(2);
                self.skip_line_break();
            }
            Ok(token)
        } else {
            self.set_scanner_error(
                "while scanning a directive",
                start_mark,
                "did not find expected comment or line break",
            )
        }
    }

    fn scan_directive_name(&mut self, start_mark: Mark) -> Result<String> {
        let mut string = String::new();
        self.input.lookahead(1);

        loop {
            if !self.input.next_is_alpha() {
                break;
            }
            self.read_char(&mut string);
            self.input.lookahead(1);
        }

        if string.is_empty() {
            self.set_scanner_error(
                "while scanning a directive",
                start_mark,
                "could not find expected directive name",
            )
        } else if !self.input.next_is_blank_or_breakz() {
            self.set_scanner_error(
                "while scanning a directive",
                start_mark,
                "found unexpected non-alphabetical character",
            )
        } else {
            Ok(string)
        }
    }

    fn scan_version_directive_value(
        &mut self,
        start_mark: Mark,
        major: &mut i32,
        minor: &mut i32,
    ) -> Result<()> {
        self.input.lookahead(1);
        while self.input.next_is_blank() {
            self.skip_char();
            self.input.lookahead(1);
        }
        self.scan_version_directive_number(start_mark, major)?;
        if !self.input.next_char_is('.') {
            return self.set_scanner_error(
                "while scanning a %YAML directive",
                start_mark,
                "did not find expected digit or '.' character",
            );
        }
        self.skip_char();
        self.scan_version_directive_number(start_mark, minor)
    }

    fn scan_version_directive_number(
        &mut self,
        start_mark: Mark,
        number: &mut i32,
    ) -> Result<()> {
        let mut value: i32 = 0;
        let mut length = 0;
        self.input.lookahead(1);
        while self.input.next_is_digit() {
            length += 1;
            if length > MAX_NUMBER_LENGTH {
                return self.set_scanner_error(
                    "while scanning a %YAML directive",
                    start_mark,
                    "found extremely long version number",
                );
            }
            value = (value * 10) + self.input.peek().to_digit(10).unwrap() as i32;
            self.skip_char();
            self.input.lookahead(1);
        }
        if length == 0 {
            return self.set_scanner_error(
                "while scanning a %YAML directive",
                start_mark,
                "did not find expected version number",
            );
        }
        *number = value;
        Ok(())
    }

    // Returns (handle, prefix)
    fn scan_tag_directive_value(&mut self, start_mark: Mark) -> Result<(String, String)> {
        self.input.lookahead(1);

        loop {
            if self.input.next_is_blank() {
                self.skip_char();
                self.input.lookahead(1);
            } else {
                let handle_value = self.scan_tag_handle(true, start_mark)?;

                self.input.lookahead(1);

                if !self.input.next_is_blank() {
                    return self.set_scanner_error(
                        "while scanning a %TAG directive",
                        start_mark,
                        "did not find expected whitespace",
                    );
                }

                while self.input.next_is_blank() {
                    self.skip_char();
                    self.input.lookahead(1);
                }

                let prefix_value = self.scan_tag_uri(true, true, None, start_mark)?;
                self.input.lookahead(1);

                if !self.input.next_is_blank_or_breakz() {
                    return self.set_scanner_error(
                        "while scanning a %TAG directive",
                        start_mark,
                        "did not find expected whitespace or line break",
                    );
                }
                return Ok((handle_value, prefix_value));
            }
        }
    }

    fn scan_anchor(&mut self, scan_alias_instead_of_anchor: bool) -> Result<Token> {
        let mut length: i32 = 0;

        let mut string = String::new();
        let start_mark: Mark = self.mark;
        self.skip_char();
        self.input.lookahead(1);

        loop {
            if !self.input.next_is_alpha() {
                break;
            }
            self.read_char(&mut string);
            self.input.lookahead(1);
            length += 1;
        }
        let end_mark: Mark = self.mark;
        if length == 0
            || !(self.input.next_is_blank_or_breakz()
                || self.input.next_char_is('?')
                || self.input.next_char_is(':')
                || self.input.next_char_is(',')
                || self.input.next_char_is(']')
                || self.input.next_char_is('}')
                || self.input.next_char_is('%')
                || self.input.next_char_is('@')
                || self.input.next_char_is('`'))
        {
            self.set_scanner_error(
                if scan_alias_instead_of_anchor {
                    "while scanning an alias"
                } else {
                    "while scanning an anchor"
                },
                start_mark,
                "did not find expected alphabetic or numeric character",
            )
        } else {
            Ok(Token {
                data: if scan_alias_instead_of_anchor {
                    TokenData::Alias { value: string }
                } else {
                    TokenData::Anchor { value: string }
                },
                start_mark,
                end_mark,
            })
        }
    }

    fn scan_tag(&mut self) -> Result<Token> {
        let mut handle;
        let mut suffix;

        let start_mark: Mark = self.mark;

        self.input.lookahead(2);

        if self.input.nth_char_is(1, '<') {
            handle = String::new();
            self.skip_char();
            self.skip_char();
            suffix = self.scan_tag_uri(true, false, None, start_mark)?;

            if !self.input.next_char_is('>') {
                return self.set_scanner_error(
                    "while scanning a tag",
                    start_mark,
                    "did not find the expected '>'",
                );
            }
            self.skip_char();
        } else {
            handle = self.scan_tag_handle(false, start_mark)?;
            if handle.starts_with('!') && handle.len() > 1 && handle.ends_with('!') {
                suffix = self.scan_tag_uri(false, false, None, start_mark)?;
            } else {
                suffix = self.scan_tag_uri(false, false, Some(&handle), start_mark)?;
                handle = String::from("!");
                if suffix.is_empty() {
                    core::mem::swap(&mut handle, &mut suffix);
                }
            }
        }

        self.input.lookahead(1);
        if !self.input.next_is_blank_or_breakz() {
            if self.flow_level == 0 || !self.input.next_char_is(',') {
                return self.set_scanner_error(
                    "while scanning a tag",
                    start_mark,
                    "did not find expected whitespace or line break",
                );
            }
            // In flow context, a tag can be followed by ',' without whitespace.
        }

        let end_mark: Mark = self.mark;
        Ok(Token {
            data: TokenData::Tag { handle, suffix },
            start_mark,
            end_mark,
        })
    }

    fn scan_tag_handle(&mut self, directive: bool, start_mark: Mark) -> Result<String> {
        let mut string = String::new();
        self.input.lookahead(1);

        if !self.input.next_char_is('!') {
            return self.set_scanner_error(
                if directive {
                    "while scanning a tag directive"
                } else {
                    "while scanning a tag"
                },
                start_mark,
                "did not find expected '!'",
            );
        }

        self.read_char(&mut string);
        self.input.lookahead(1);
        loop {
            if !self.input.next_is_alpha() {
                break;
            }
            self.read_char(&mut string);
            self.input.lookahead(1);
        }
        if self.input.next_char_is('!') {
            self.read_char(&mut string);
        } else if directive && string != "!" {
            return self.set_scanner_error(
                "while parsing a tag directive",
                start_mark,
                "did not find expected '!'",
            );
        }
        Ok(string)
    }

    fn scan_tag_uri(
        &mut self,
        uri_char: bool,
        directive: bool,
        head: Option<&str>,
        start_mark: Mark,
    ) -> Result<String> {
        let head = head.unwrap_or("");
        let mut length = head.len();
        let mut string = String::new();

        if length > 1 {
            string = String::from(&head[1..]);
        }
        self.input.lookahead(1);

        while self.input.next_is_alpha()
            || self.input.next_char_is(';')
            || self.input.next_char_is('/')
            || self.input.next_char_is('?')
            || self.input.next_char_is(':')
            || self.input.next_char_is('@')
            || self.input.next_char_is('&')
            || self.input.next_char_is('=')
            || self.input.next_char_is('+')
            || self.input.next_char_is('$')
            || self.input.next_char_is('.')
            || self.input.next_char_is('%')
            || self.input.next_char_is('!')
            || self.input.next_char_is('~')
            || self.input.next_char_is('*')
            || self.input.next_char_is('\'')
            || self.input.next_char_is('(')
            || self.input.next_char_is(')')
            || uri_char
                && (self.input.next_char_is(',')
                    || self.input.next_char_is('[')
                    || self.input.next_char_is(']'))
        {
            if self.input.next_char_is('%') {
                self.scan_uri_escapes(directive, start_mark, &mut string)?;
            } else {
                self.read_char(&mut string);
            }
            length += 1;
            self.input.lookahead(1);
        }
        if length == 0 {
            self.set_scanner_error(
                if directive {
                    "while parsing a %TAG directive"
                } else {
                    "while parsing a tag"
                },
                start_mark,
                "did not find expected tag URI",
            )
        } else {
            Ok(string)
        }
    }

    fn scan_uri_escapes(
        &mut self,
        directive: bool,
        start_mark: Mark,
        string: &mut String,
    ) -> Result<()> {
        let mut width: i32 = 0;
        loop {
            self.input.lookahead(3);
            if !(self.input.next_char_is('%')
                && is_hex(self.input.peek_nth(1))
                && is_hex(self.input.peek_nth(2)))
            {
                return self.set_scanner_error(
                    if directive {
                        "while parsing a %TAG directive"
                    } else {
                        "while parsing a tag"
                    },
                    start_mark,
                    "did not find URI escaped octet",
                );
            }
            let octet =
                ((as_hex(self.input.peek_nth(1)) << 4) + as_hex(self.input.peek_nth(2))) as u8;
            if width == 0 {
                width = if octet & 0x80 == 0 {
                    1
                } else if octet & 0xE0 == 0xC0 {
                    2
                } else if octet & 0xF0 == 0xE0 {
                    3
                } else if octet & 0xF8 == 0xF0 {
                    4
                } else {
                    0
                };
                // TODO: Something is fishy here, why isn't `width` being used?
                if width == 0 {
                    return self.set_scanner_error(
                        if directive {
                            "while parsing a %TAG directive"
                        } else {
                            "while parsing a tag"
                        },
                        start_mark,
                        "found an incorrect leading UTF-8 octet",
                    );
                }
            } else if octet & 0xC0 != 0x80 {
                return self.set_scanner_error(
                    if directive {
                        "while parsing a %TAG directive"
                    } else {
                        "while parsing a tag"
                    },
                    start_mark,
                    "found an incorrect trailing UTF-8 octet",
                );
            }
            string.push(char::from_u32(octet as _).expect("invalid Unicode"));
            self.skip_char();
            self.skip_char();
            self.skip_char();
            width -= 1;
            if width == 0 {
                break;
            }
        }
        Ok(())
    }

    fn scan_block_scalar(&mut self, literal: bool) -> Result<Token> {
        let mut end_mark: Mark;
        let mut string = String::new();
        let mut leading_break = String::new();
        let mut trailing_breaks = String::new();
        let mut chomping: i32 = 0;
        let mut increment: i32 = 0;
        let mut indent: i32 = 0;
        let mut leading_blank: i32 = 0;
        let mut trailing_blank: i32;
        let start_mark: Mark = self.mark;
        self.skip_char();
        self.input.lookahead(1);

        if self.input.next_char_is('+') || self.input.next_char_is('-') {
            chomping = if self.input.next_char_is('+') {
                1
            } else {
                -1
            };
            self.skip_char();
            self.input.lookahead(1);
            if self.input.next_is_digit() {
                if self.input.next_char_is('0') {
                    return self.set_scanner_error(
                        "while scanning a block scalar",
                        start_mark,
                        "found an indentation indicator equal to 0",
                    );
                }
                increment = self.input.peek().to_digit(10).unwrap() as i32;
                self.skip_char();
            }
        } else if self.input.next_is_digit() {
            if self.input.next_char_is('0') {
                return self.set_scanner_error(
                    "while scanning a block scalar",
                    start_mark,
                    "found an indentation indicator equal to 0",
                );
            }
            increment = self.input.peek().to_digit(10).unwrap() as i32;
            self.skip_char();
            self.input.lookahead(1);
            if self.input.next_char_is('+') || self.input.next_char_is('-') {
                chomping = if self.input.next_char_is('+') {
                    1
                } else {
                    -1
                };
                self.skip_char();
            }
        }

        self.input.lookahead(1);
        loop {
            if !self.input.next_is_blank() {
                break;
            }
            self.skip_char();
            self.input.lookahead(1);
        }

        if self.input.next_char_is('#') {
            loop {
                if self.input.next_is_breakz() {
                    break;
                }
                self.skip_char();
                self.input.lookahead(1);
            }
        }

        if !self.input.next_is_breakz() {
            return self.set_scanner_error(
                "while scanning a block scalar",
                start_mark,
                "did not find expected comment or line break",
            );
        }

        if self.input.next_is_break() {
            self.input.lookahead(2);
            self.skip_line_break();
        }

        end_mark = self.mark;
        if increment != 0 {
            indent = if self.indent >= 0 {
                self.indent + increment
            } else {
                increment
            };
        }
        self.scan_block_scalar_breaks(
            &mut indent,
            &mut trailing_breaks,
            start_mark,
            &mut end_mark,
        )?;

        self.input.lookahead(1);

        loop {
            if self.mark.column as i32 != indent || self.input.next_is_z() {
                break;
            }
            trailing_blank = self.input.next_is_blank() as i32;
            if !literal
                && leading_break.starts_with('\n')
                && leading_blank == 0
                && trailing_blank == 0
            {
                if trailing_breaks.is_empty() {
                    string.push(' ');
                }
                leading_break.clear();
            } else {
                string.push_str(&leading_break);
                leading_break.clear();
            }
            string.push_str(&trailing_breaks);
            trailing_breaks.clear();
            leading_blank = self.input.next_is_blank() as i32;
            while !self.input.next_is_breakz() {
                self.read_char(&mut string);
                self.input.lookahead(1);
            }
            self.input.lookahead(2);
            self.read_line_break(&mut leading_break);
            self.scan_block_scalar_breaks(
                &mut indent,
                &mut trailing_breaks,
                start_mark,
                &mut end_mark,
            )?;
        }

        if chomping != -1 {
            string.push_str(&leading_break);
        }

        if chomping == 1 {
            string.push_str(&trailing_breaks);
        }

        Ok(Token {
            data: TokenData::Scalar {
                value: string,
                style: if literal {
                    ScalarStyle::Literal
                } else {
                    ScalarStyle::Folded
                },
            },
            start_mark,
            end_mark,
        })
    }

    fn scan_block_scalar_breaks(
        &mut self,
        indent: &mut i32,
        breaks: &mut String,
        start_mark: Mark,
        end_mark: &mut Mark,
    ) -> Result<()> {
        let mut max_indent: i32 = 0;
        *end_mark = self.mark;
        loop {
            self.input.lookahead(1);
            while (*indent == 0 || (self.mark.column as i32) < *indent)
                && self.input.next_char_is(' ')
            {
                self.skip_char();
                self.input.lookahead(1);
            }
            if self.mark.column as i32 > max_indent {
                max_indent = self.mark.column as i32;
            }
            if (*indent == 0 || (self.mark.column as i32) < *indent)
                && self.input.next_char_is('\t')
            {
                return self.set_scanner_error(
                    "while scanning a block scalar",
                    start_mark,
                    "found a tab character where an indentation space is expected",
                );
            }
            if !self.input.next_is_break() {
                break;
            }
            self.input.lookahead(2);
            self.read_line_break(breaks);
            *end_mark = self.mark;
        }
        if *indent == 0 {
            *indent = max_indent;
            if *indent < self.indent + 1 {
                *indent = self.indent + 1;
            }
            if *indent < 1 {
                *indent = 1;
            }
        }
        Ok(())
    }

    fn scan_flow_scalar(&mut self, single: bool) -> Result<Token> {
        let mut string = String::new();
        let mut leading_break = String::new();
        let mut trailing_breaks = String::new();
        let mut whitespaces = String::new();
        let mut leading_blanks;

        let start_mark: Mark = self.mark;
        self.skip_char();
        loop {
            self.input.lookahead(4);

            if self.mark.column == 0
                && (self.input.nth_char_is(0, '-')
                    && self.input.nth_char_is(1, '-')
                    && self.input.nth_char_is(2, '-')
                    || self.input.nth_char_is(0, '.')
                        && self.input.nth_char_is(1, '.')
                        && self.input.nth_char_is(2, '.'))
                && is_blank_or_breakz(self.input.peek_nth(3))
            {
                return self.set_scanner_error(
                    "while scanning a quoted scalar",
                    start_mark,
                    "found unexpected document indicator",
                );
            } else if self.input.next_is_z() {
                return self.set_scanner_error(
                    "while scanning a quoted scalar",
                    start_mark,
                    "found unexpected end of stream",
                );
            }
            self.input.lookahead(2);
            leading_blanks = false;
            while !self.input.next_is_blank_or_breakz() {
                if single
                    && self.input.nth_char_is(0, '\'')
                    && self.input.nth_char_is(1, '\'')
                {
                    string.push('\'');
                    self.skip_char();
                    self.skip_char();
                } else {
                    if self
                        .input
                        .next_char_is(if single { '\'' } else { '"' })
                    {
                        break;
                    }
                    if !single
                        && self.input.next_char_is('\\')
                        && is_break(self.input.peek_nth(1))
                    {
                        self.input.lookahead(3);
                        self.skip_char();
                        self.skip_line_break();
                        leading_blanks = true;
                        break;
                    } else if !single && self.input.next_char_is('\\') {
                        let mut code_length = 0usize;
                        match self.input.peek_nth(1) {
                            '0' => {
                                string.push('\0');
                            }
                            'a' => {
                                string.push('\x07');
                            }
                            'b' => {
                                string.push('\x08');
                            }
                            't' | '\t' => {
                                string.push('\t');
                            }
                            'n' => {
                                string.push('\n');
                            }
                            'v' => {
                                string.push('\x0B');
                            }
                            'f' => {
                                string.push('\x0C');
                            }
                            'r' => {
                                string.push('\r');
                            }
                            'e' => {
                                string.push('\x1B');
                            }
                            ' ' => {
                                string.push(' ');
                            }
                            '"' => {
                                string.push('"');
                            }
                            '/' => {
                                string.push('/');
                            }
                            '\\' => {
                                string.push('\\');
                            }
                            // NEL (#x85)
                            'N' => {
                                string.push('\u{0085}');
                            }
                            // #xA0
                            '_' => {
                                string.push('\u{00a0}');
                            }
                            // LS (#x2028)
                            'L' => {
                                string.push('\u{2028}');
                            }
                            // PS (#x2029)
                            'P' => {
                                string.push('\u{2029}');
                            }
                            'x' => {
                                code_length = 2;
                            }
                            'u' => {
                                code_length = 4;
                            }
                            'U' => {
                                code_length = 8;
                            }
                            _ => {
                                return self.set_scanner_error(
                                    "while parsing a quoted scalar",
                                    start_mark,
                                    "found unknown escape character",
                                );
                            }
                        }
                        self.skip_char();
                        self.skip_char();
                        if code_length != 0 {
                            let mut value: u32 = 0;
                            let mut k = 0;
                            self.input.lookahead(code_length);
                            while k < code_length {
                                if !is_hex(self.input.peek_nth(k)) {
                                    return self.set_scanner_error(
                                        "while parsing a quoted scalar",
                                        start_mark,
                                        "did not find expected hexdecimal number",
                                    );
                                }
                                value = (value << 4) + as_hex(self.input.peek_nth(k));
                                k += 1;
                            }
                            if let Some(ch) = char::from_u32(value) {
                                string.push(ch);
                            } else {
                                return self.set_scanner_error(
                                    "while parsing a quoted scalar",
                                    start_mark,
                                    "found invalid Unicode character escape code",
                                );
                            }

                            k = 0;
                            while k < code_length {
                                self.skip_char();
                                k += 1;
                            }
                        }
                    } else {
                        self.read_char(&mut string);
                    }
                }
                self.input.lookahead(2);
            }
            self.input.lookahead(1);
            if self
                .input
                .next_char_is(if single { '\'' } else { '"' })
            {
                break;
            }
            self.input.lookahead(1);
            while self.input.next_is_blank() || self.input.next_is_break() {
                if self.input.next_is_blank() {
                    if leading_blanks {
                        self.skip_char();
                    } else {
                        self.read_char(&mut whitespaces);
                    }
                } else {
                    self.input.lookahead(2);
                    if leading_blanks {
                        self.read_line_break(&mut trailing_breaks);
                    } else {
                        whitespaces.clear();
                        self.read_line_break(&mut leading_break);
                        leading_blanks = true;
                    }
                }
                self.input.lookahead(1);
            }
            if leading_blanks {
                if leading_break.starts_with('\n') {
                    if trailing_breaks.is_empty() {
                        string.push(' ');
                    } else {
                        string.push_str(&trailing_breaks);
                        trailing_breaks.clear();
                    }
                    leading_break.clear();
                } else {
                    string.push_str(&leading_break);
                    string.push_str(&trailing_breaks);
                    leading_break.clear();
                    trailing_breaks.clear();
                }
            } else {
                string.push_str(&whitespaces);
                whitespaces.clear();
            }
        }

        self.skip_char();
        let end_mark: Mark = self.mark;
        Ok(Token {
            data: TokenData::Scalar {
                value: string,
                style: if single {
                    ScalarStyle::SingleQuoted
                } else {
                    ScalarStyle::DoubleQuoted
                },
            },
            start_mark,
            end_mark,
        })
    }

    fn scan_plain_scalar(&mut self) -> Result<Token> {
        let mut end_mark: Mark;
        let mut string = String::new();
        let mut leading_break = String::new();
        let mut trailing_breaks = String::new();
        let mut whitespaces = String::new();
        let mut leading_blanks = false;
        let indent: i32 = self.indent + 1;
        end_mark = self.mark;
        let start_mark: Mark = end_mark;
        loop {
            self.input.lookahead(4);
            if self.mark.column == 0
                && (self.input.nth_char_is(0, '-')
                    && self.input.nth_char_is(1, '-')
                    && self.input.nth_char_is(2, '-')
                    || self.input.nth_char_is(0, '.')
                        && self.input.nth_char_is(1, '.')
                        && self.input.nth_char_is(2, '.'))
                && is_blank_or_breakz(self.input.peek_nth(3))
            {
                break;
            }
            if self.input.next_char_is('#') {
                break;
            }
            while !self.input.next_is_blank_or_breakz() {
                if self.flow_level != 0
                    && self.input.next_char_is(':')
                    && (self.input.nth_char_is(1, ',')
                        || self.input.nth_char_is(1, '?')
                        || self.input.nth_char_is(1, '[')
                        || self.input.nth_char_is(1, ']')
                        || self.input.nth_char_is(1, '{')
                        || self.input.nth_char_is(1, '}'))
                {
                    return self.set_scanner_error(
                        "while scanning a plain scalar",
                        start_mark,
                        "found unexpected ':'",
                    );
                }

                if self.input.next_char_is(':')
                    && is_blank_or_breakz(self.input.peek_nth(1))
                    || self.flow_level != 0
                        && (self.input.next_char_is(',')
                            || self.input.next_char_is('[')
                            || self.input.next_char_is(']')
                            || self.input.next_char_is('{')
                            || self.input.next_char_is('}'))
                {
                    break;
                }
                if leading_blanks || !whitespaces.is_empty() {
                    if leading_blanks {
                        if leading_break.starts_with('\n') {
                            if trailing_breaks.is_empty() {
                                string.push(' ');
                            } else {
                                string.push_str(&trailing_breaks);
                                trailing_breaks.clear();
                            }
                            leading_break.clear();
                        } else {
                            string.push_str(&leading_break);
                            string.push_str(&trailing_breaks);
                            leading_break.clear();
                            trailing_breaks.clear();
                        }
                        leading_blanks = false;
                    } else {
                        string.push_str(&whitespaces);
                        whitespaces.clear();
                    }
                }
                self.read_char(&mut string);
                end_mark = self.mark;
                self.input.lookahead(2);
            }
            if !(self.input.next_is_blank() || self.input.next_is_break()) {
                break;
            }
            self.input.lookahead(1);

            while self.input.next_is_blank() || self.input.next_is_break() {
                if self.input.next_is_blank() {
                    if leading_blanks
                        && (self.mark.column as i32) < indent
                        && self.input.next_char_is('\t')
                    {
                        return self.set_scanner_error(
                            "while scanning a plain scalar",
                            start_mark,
                            "found a tab character that violates indentation",
                        );
                    } else if !leading_blanks {
                        self.read_char(&mut whitespaces);
                    } else {
                        self.skip_char();
                    }
                } else {
                    self.input.lookahead(2);

                    if leading_blanks {
                        self.read_line_break(&mut trailing_breaks);
                    } else {
                        whitespaces.clear();
                        self.read_line_break(&mut leading_break);
                        leading_blanks = true;
                    }
                }
                self.input.lookahead(1);
            }
            if self.flow_level == 0 && (self.mark.column as i32) < indent {
                break;
            }
        }

        if leading_blanks {
            self.simple_key_allowed = true;
        }

        Ok(Token {
            data: TokenData::Scalar {
                value: string,
                style: ScalarStyle::Plain,
            },
            start_mark,
            end_mark,
        })
    }
}

impl<I: Input> Iterator for Scanner<I> {
    type Item = Result<Token>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stream_end_produced {
            None
        } else {
            Some(self.scan())
        }
    }
}

impl<I: Input> core::iter::FusedIterator for Scanner<I> {}

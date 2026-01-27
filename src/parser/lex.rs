//! Simple lexer with 1-token lookahead that handles a subset of C relevant to classic bytebeat. Only intended for a single statement of 1+ expression.
//!
//! Column and line aware.
use std::{i32, iter::Peekable, num::IntErrorKind, str::Chars};

use tracing::warn;

use crate::parser::{Column, LexError, Line, Operator, Span, Spanned, Token};

pub struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
    // Used to create spans for tokens
    // If we enumerate chars it's not peekable anymore!
    line: Line,
    col: Column,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Lexer<'a> {
        Lexer {
            chars: input.chars().peekable(),
            line: 0,
            col: 0,
        }
    }

    /// Advances the iterator and increments position or line counter.
    fn bump(&mut self) -> Option<char> {
        match self.chars.next() {
            Some(c) => {
                match c {
                    '\n' => {
                        self.line += 1;
                        self.col = 0;
                    }
                    // Still advance, but don't affect span. Lazy way to avoid lookahead here
                    '\r' => {}
                    // No tab support, unicode linebreaks, etc.
                    _ => {
                        self.col += 1;
                    }
                }
                Some(c)
            }
            None => None,
        }
    }

    pub fn next(&mut self) -> Spanned<Token> {
        self.skip_whitespace();
        let (start_line, start_col) = (self.line, self.col);
        let token = match self.chars.peek() {
            Some(&c) => {
                match c {
                    // I: Operators, including multi-char
                    '+' => {
                        self.bump();
                        Token::Op(Operator::Plus)
                    }
                    '-' => {
                        self.bump();
                        Token::Op(Operator::Minus)
                    }
                    '/' => {
                        self.bump();
                        Token::Op(Operator::Div)
                    }
                    '*' => {
                        self.bump();
                        Token::Op(Operator::Mul)
                    }
                    '%' => {
                        self.bump();
                        Token::Op(Operator::Mod)
                    }
                    '&' => {
                        self.bump();
                        if let Some('&') = self.chars.peek() {
                            self.bump();
                            Token::Op(Operator::LogAnd)
                        } else {
                            Token::Op(Operator::And)
                        }
                    }
                    '|' => {
                        self.bump();
                        if let Some('|') = self.chars.peek() {
                            self.bump();
                            Token::Op(Operator::LogOr)
                        } else {
                            Token::Op(Operator::Or)
                        }
                    }
                    '^' => {
                        self.bump();
                        Token::Op(Operator::BitXor)
                    }
                    '~' => {
                        self.bump();
                        Token::Op(Operator::BitNot)
                    }
                    '!' => {
                        self.bump();
                        if let Some('=') = self.chars.peek() {
                            self.bump();
                            Token::Op(Operator::Ne)
                        } else {
                            Token::Op(Operator::LogNot)
                        }
                    }
                    '=' => {
                        self.bump();
                        if let Some('=') = self.chars.peek() {
                            self.bump();
                            Token::Op(Operator::Eq)
                        } else {
                            // Hey pal, this isn't that kind of statement!
                            Token::Err(LexError::SolitaryEquals)
                        }
                    }
                    '?' => {
                        self.bump();
                        Token::Op(Operator::Question)
                    }
                    ':' => {
                        self.bump();
                        Token::Op(Operator::Colon)
                    }
                    '(' => {
                        self.bump();
                        Token::Op(Operator::Lparen)
                    }
                    ')' => {
                        self.bump();
                        Token::Op(Operator::Rparen)
                    }
                    '<' => {
                        self.bump(); // consume first <
                        if let Some(&next) = self.chars.peek() {
                            if next == '<' {
                                self.bump();
                                Token::Op(Operator::Lsh)
                            } else if next == '=' {
                                self.bump();
                                Token::Op(Operator::Le)
                            } else {
                                Token::Op(Operator::Lt)
                            }
                        } else {
                            Token::Op(Operator::Lt)
                        }
                    }
                    '>' => {
                        self.bump();
                        if let Some(&next) = self.chars.peek() {
                            if next == '>' {
                                self.bump();
                                Token::Op(Operator::Rsh)
                            } else if next == '=' {
                                self.bump();
                                Token::Op(Operator::Ge)
                            } else {
                                Token::Op(Operator::Gt)
                            }
                        } else {
                            Token::Op(Operator::Gt)
                        }
                    }
                    // II: Numbers (always into i32) may use C-syntax for base 2, 8, 10, 16
                    '0' => {
                        // Leading 0 may be 0 or a base that's not 10
                        self.bump();
                        if let Some(&next) = self.chars.peek() {
                            if next == 'x' {
                                self.bump();
                                self.lex_number(16)
                            } else if next == 'b' {
                                self.bump();
                                self.lex_number(2)
                            } else if next.is_ascii_digit() {
                                self.lex_number(8)
                            } else {
                                // 0 and then another token
                                Token::Number(0)
                            }
                        } else {
                            // 0 and then EOF
                            Token::Number(0)
                        }
                    }
                    '1'..='9' => self.lex_number(10),
                    // III: Variables. Could be anything, but we restrict to 't' for the users' sanity.
                    't' => {
                        self.bump();
                        Token::Variable
                    }
                    _ => {
                        self.bump();
                        Token::Err(LexError::UnexpectedChar(c))
                    }
                }
            }
            // IV: End
            None => Token::Eof,
        };

        // Every arm besides Eof does a bump, so the end of THIS token is actually last column
        let (end_line, end_col) = if self.col > start_col {
            (self.line, self.col.saturating_sub(1))
        } else {
            (start_line, start_col)
        };

        // We should not have any multi-line tokens
        debug_assert!(start_line == end_line);

        Spanned::new(token, Span::new(start_line, start_col, end_col))
    }

    /// Skips unicode whitespace and both line-endings (\r\n, \n), not equipped for bizarre unicode linebreaks and etc.
    fn skip_whitespace(&mut self) {
        while let Some(&peeked) = self.chars.peek() {
            if peeked.is_whitespace() || peeked == '\r' || peeked == '\n' {
                self.bump();
            } else {
                return;
            }
        }
    }

    /// More robust helper for lexing ASCII numbers with any base. Must be called after separators removed
    fn lex_number(&mut self, radix: u32) -> Token {
        let mut digits = String::new();
        while let Some(&peeked) = self.chars.peek() {
            if peeked.is_digit(radix) {
                digits.push(self.bump().unwrap());
            } else {
                break;
            }
        }

        i32::from_str_radix(&digits, radix).map_or_else(
            |e| match e.kind() {
                IntErrorKind::PosOverflow => {
                    warn!("lexer is setting overflowing number {digits} to i32 max");
                    Token::Number(i32::MAX)
                }
                IntErrorKind::NegOverflow => {
                    unreachable!() // Parser does negation. `-` is a prefix operator
                }
                IntErrorKind::Zero => Token::Number(0),
                IntErrorKind::Empty | IntErrorKind::InvalidDigit | _ => {
                    Token::Err(LexError::ImproperNumber(radix as i8, e))
                }
            },
            |n| Token::Number(n),
        )
    }
}

// Mostly focused on verifying span positions and number logic
// More than strictly necessary because these are originally slopped, but they have reasonable coverage
#[cfg(test)]
mod tests {
    use super::*;

    // Not used for ImproperNumber because we can't `_` the internal error v0v
    fn assert_token(
        lexer: &mut Lexer,
        expected_token: Token,
        line: Line,
        start: Column,
        end: Column,
    ) {
        let spanned = lexer.next();
        assert_eq!(
            spanned.node, expected_token,
            "Token mismatch at line {} col {}..{}",
            line, start, end
        );
        assert_eq!(spanned.span.line, line, "Line mismatch");
        assert_eq!(spanned.span.start, start, "Start column mismatch");
        assert_eq!(spanned.span.end, end, "End column mismatch");
    }

    // Entirely 1-char lexemes without whitespace
    #[test]
    fn test_single_char_no_whitespace() {
        let input = "t+t";
        let mut lexer = Lexer::new(input);

        // 't' at 0..1 (len 1) -> line 0, 0..0
        assert_token(&mut lexer, Token::Variable, 0, 0, 0);
        // '+' at 1..2 (len 1) -> line 0, 1..1
        assert_token(&mut lexer, Token::Op(Operator::Plus), 0, 1, 1);
        // 't' at 2..3 (len 1) -> line 0, 2..2
        assert_token(&mut lexer, Token::Variable, 0, 2, 2);

        assert_token(&mut lexer, Token::Eof, 0, 3, 3);
    }

    // 1-char lexemes with whitespace
    #[test]
    fn test_single_char_with_whitespace() {
        let input = "t + t";
        let mut lexer = Lexer::new(input);

        // 't' at 0
        assert_token(&mut lexer, Token::Variable, 0, 0, 0);
        // ' ' at 1 (skip)
        // '+' at 2
        assert_token(&mut lexer, Token::Op(Operator::Plus), 0, 2, 2);
        // ' ' at 3 (skip)
        // 't' at 4
        assert_token(&mut lexer, Token::Variable, 0, 4, 4);

        assert_token(&mut lexer, Token::Eof, 0, 5, 5);
    }

    // multi-char lexemes with whitespace
    #[test]
    fn test_multi_char_lexemes() {
        let input = "123 == 45";
        let mut lexer = Lexer::new(input);

        assert_token(&mut lexer, Token::Number(123), 0, 0, 2);
        assert_token(&mut lexer, Token::Op(Operator::Eq), 0, 4, 5);
        assert_token(&mut lexer, Token::Number(45), 0, 7, 8);
        assert_token(&mut lexer, Token::Eof, 0, 9, 9);
    }

    #[test]
    fn test_eof_empty() {
        let input = "";
        let mut lexer = Lexer::new(input);
        assert_token(&mut lexer, Token::Eof, 0, 0, 0);
    }

    #[test]
    fn test_whitespace_only() {
        let input = "   ";
        let mut lexer = Lexer::new(input);
        assert_token(&mut lexer, Token::Eof, 0, 3, 3);
    }

    #[test]
    fn test_newlines_lf() {
        let input = "t\nt";
        let mut lexer = Lexer::new(input);

        // 't' (0,0)
        assert_token(&mut lexer, Token::Variable, 0, 0, 0);
        // '\n' increments line, resets col
        // 't' (1,0)
        assert_token(&mut lexer, Token::Variable, 1, 0, 0);
        assert_token(&mut lexer, Token::Eof, 1, 1, 1);
    }

    #[test]
    fn test_newlines_crlf() {
        let input = "t\r\nt";
        let mut lexer = Lexer::new(input);

        // 't' (0,0)
        assert_token(&mut lexer, Token::Variable, 0, 0, 0);
        // '\r' ignored (no col advance), '\n' increments line, resets col
        assert_token(&mut lexer, Token::Variable, 1, 0, 0);
        assert_token(&mut lexer, Token::Eof, 1, 1, 1);
    }

    #[test]
    fn test_tabs() {
        // Tabs are treated as single-column v0v
        let input = "t\tt";
        let mut lexer = Lexer::new(input);

        // 't' (0,0)
        assert_token(&mut lexer, Token::Variable, 0, 0, 0);
        // 't' (again) starts at (0,2).
        assert_token(&mut lexer, Token::Variable, 0, 2, 2);
        assert_token(&mut lexer, Token::Eof, 0, 3, 3);
    }

    #[test]
    fn test_mixed_whitespace_multiline() {
        let input = "t \n  t";
        let mut lexer = Lexer::new(input);

        // 't' (0,0). Ends at (0,0). POS is (0,1).
        assert_token(&mut lexer, Token::Variable, 0, 0, 0);

        // 't' starts (1,2). Ends (1,2). Pos (1,3).
        assert_token(&mut lexer, Token::Variable, 1, 2, 2);
        assert_token(&mut lexer, Token::Eof, 1, 3, 3);
    }

    #[test]
    fn test_no_tokens_across_newline() {
        // This should be two bitwise Or instead of one Logical Or
        let input = "t |\n| 5";
        let mut lexer = Lexer::new(input);

        assert_token(&mut lexer, Token::Variable, 0, 0, 0);
        assert_token(&mut lexer, Token::Op(Operator::Or), 0, 2, 2);
        assert_token(&mut lexer, Token::Op(Operator::Or), 1, 0, 0);
        assert_token(&mut lexer, Token::Number(5), 1, 2, 2);
        assert_token(&mut lexer, Token::Eof, 1, 3, 3);
    }

    #[test]
    fn test_newline_at_end() {
        // Needed to convince myself the column counter logic was okay
        let input = "t+5\n&&10\n";
        let mut lexer = Lexer::new(input);

        assert_token(&mut lexer, Token::Variable, 0, 0, 0);
        assert_token(&mut lexer, Token::Op(Operator::Plus), 0, 1, 1);
        assert_token(&mut lexer, Token::Number(5), 0, 2, 2);
        assert_token(&mut lexer, Token::Op(Operator::LogAnd), 1, 0, 1);
        assert_token(&mut lexer, Token::Number(10), 1, 2, 3);
        assert_token(&mut lexer, Token::Eof, 2, 0, 0);
    }

    #[test]
    fn test_error_tokens() {
        let input = "=";
        let mut lexer = Lexer::new(input);

        assert_token(&mut lexer, Token::Err(LexError::SolitaryEquals), 0, 0, 0);

        let input = "@";
        let mut lexer = Lexer::new(input);

        assert_token(
            &mut lexer,
            Token::Err(LexError::UnexpectedChar('@')),
            0,
            0,
            0,
        );
    }

    // ==================== Base 10 (Decimal) Tests ====================

    #[test]
    fn test_decimal_single_digit() {
        let mut lexer = Lexer::new("5");
        assert_token(&mut lexer, Token::Number(5), 0, 0, 0);
        assert_token(&mut lexer, Token::Eof, 0, 1, 1);
    }

    #[test]
    fn test_decimal_multi_digit() {
        let mut lexer = Lexer::new("123");
        assert_token(&mut lexer, Token::Number(123), 0, 0, 2);
        assert_token(&mut lexer, Token::Eof, 0, 3, 3);
    }

    #[test]
    fn test_decimal_overflow() {
        let mut lexer = Lexer::new("9999999999999999999");
        assert_token(&mut lexer, Token::Number(i32::MAX), 0, 0, 18);
    }

    #[test]
    fn test_decimal_followed_by_non_digit() {
        let mut lexer = Lexer::new("42+t");
        assert_token(&mut lexer, Token::Number(42), 0, 0, 1);
        assert_token(&mut lexer, Token::Op(Operator::Plus), 0, 2, 2);
        assert_token(&mut lexer, Token::Variable, 0, 3, 3);
    }

    #[test]
    fn test_decimal_max_i32() {
        let mut lexer = Lexer::new("2147483647");
        assert_token(&mut lexer, Token::Number(i32::MAX), 0, 0, 9);
    }

    #[test]
    fn test_decimal_one_above_max() {
        let mut lexer = Lexer::new("2147483648");
        assert_token(&mut lexer, Token::Number(i32::MAX), 0, 0, 9);
    }

    // ==================== Base 16 (Hexadecimal) Tests ====================

    #[test]
    fn test_hex_simple() {
        let mut lexer = Lexer::new("0xff");
        assert_token(&mut lexer, Token::Number(255), 0, 0, 3);
    }

    #[test]
    fn test_hex_mixed_case() {
        let mut lexer = Lexer::new("0xAbCdEf");
        assert_token(&mut lexer, Token::Number(0xABCDEF), 0, 0, 7);
    }

    #[test]
    fn test_hex_all_digit_types() {
        let mut lexer = Lexer::new("0x1a2b3c");
        assert_token(&mut lexer, Token::Number(0x1a2b3c), 0, 0, 7);
    }

    #[test]
    fn test_hex_overflow() {
        let mut lexer = Lexer::new("0xFFFFFFFF");
        assert_token(&mut lexer, Token::Number(i32::MAX), 0, 0, 9);
    }

    #[test]
    fn test_hex_empty_digits() {
        let mut lexer = Lexer::new("0x+");
        let spanned = lexer.next();
        assert!(matches!(
            spanned.node,
            Token::Err(LexError::ImproperNumber(16, _))
        ));
    }

    #[test]
    fn test_hex_empty_at_eof() {
        let mut lexer = Lexer::new("0x");
        let spanned = lexer.next();
        assert!(matches!(
            spanned.node,
            Token::Err(LexError::ImproperNumber(16, _))
        ));
    }

    #[test]
    fn test_hex_invalid_char_after_prefix() {
        let mut lexer = Lexer::new("0xGHI");
        let spanned = lexer.next();
        assert!(matches!(
            spanned.node,
            Token::Err(LexError::ImproperNumber(16, _))
        ));
    }

    #[test]
    fn test_hex_max_i32() {
        let mut lexer = Lexer::new("0x7FFFFFFF");
        assert_token(&mut lexer, Token::Number(i32::MAX), 0, 0, 9);
    }

    #[test]
    fn test_hex_one_above_max() {
        let mut lexer = Lexer::new("0x80000000");
        assert_token(&mut lexer, Token::Number(i32::MAX), 0, 0, 9);
    }

    // ==================== Base 2 (Binary) Tests ====================

    #[test]
    fn test_binary_simple() {
        let mut lexer = Lexer::new("0b1010");
        assert_token(&mut lexer, Token::Number(10), 0, 0, 5);
    }

    #[test]
    fn test_binary_only_zeros() {
        let mut lexer = Lexer::new("0b0000");
        assert_token(&mut lexer, Token::Number(0), 0, 0, 5);
    }

    #[test]
    fn test_binary_overflow() {
        // 33 ones will overflow i32
        let mut lexer = Lexer::new("0b111111111111111111111111111111111");
        assert_token(&mut lexer, Token::Number(i32::MAX), 0, 0, 34);
    }

    #[test]
    fn test_binary_empty_digits() {
        let mut lexer = Lexer::new("0b+");
        let spanned = lexer.next();
        assert!(matches!(
            spanned.node,
            Token::Err(LexError::ImproperNumber(2, _))
        ));
    }

    #[test]
    fn test_binary_empty_at_eof() {
        let mut lexer = Lexer::new("0b");
        let spanned = lexer.next();
        assert!(matches!(
            spanned.node,
            Token::Err(LexError::ImproperNumber(2, _))
        ));
    }

    #[test]
    fn test_binary_invalid_char_after_prefix() {
        let mut lexer = Lexer::new("0b2");
        let spanned = lexer.next();
        assert!(matches!(
            spanned.node,
            Token::Err(LexError::ImproperNumber(2, _))
        ));
    }

    // ==================== Base 8 (Octal) Tests ====================

    #[test]
    fn test_octal_simple() {
        let mut lexer = Lexer::new("0755");
        assert_token(&mut lexer, Token::Number(493), 0, 0, 3);
    }

    #[test]
    fn test_octal_overflow() {
        let mut lexer = Lexer::new("077777777777777");
        assert_token(&mut lexer, Token::Number(i32::MAX), 0, 0, 14);
    }

    #[test]
    fn test_octal_invalid_digit() {
        // '8' is not a valid octal digit, so only '0' is consumed
        let mut lexer = Lexer::new("089");
        // The leading 0 is consumed, then '8' is not octal so we get 0 (from IntErrorKind::Empty since no digits collected)
        let spanned = lexer.next();
        assert!(matches!(
            spanned.node,
            Token::Err(LexError::ImproperNumber(8, _))
        ));
    }

    #[test]
    fn test_octal_zero_padding() {
        let mut lexer = Lexer::new("007");
        assert_token(&mut lexer, Token::Number(7), 0, 0, 2);
    }

    // ==================== Literal Zero Tests ====================

    #[test]
    fn test_zero_at_eof() {
        let mut lexer = Lexer::new("0");
        assert_token(&mut lexer, Token::Number(0), 0, 0, 0);
        assert_token(&mut lexer, Token::Eof, 0, 1, 1);
    }

    #[test]
    fn test_zero_followed_by_operator() {
        let mut lexer = Lexer::new("0+");
        assert_token(&mut lexer, Token::Number(0), 0, 0, 0);
        assert_token(&mut lexer, Token::Op(Operator::Plus), 0, 1, 1);
    }

    #[test]
    fn test_zero_followed_by_space_then_digit() {
        let mut lexer = Lexer::new("0 5");
        assert_token(&mut lexer, Token::Number(0), 0, 0, 0);
        assert_token(&mut lexer, Token::Number(5), 0, 2, 2);
    }

    #[test]
    fn test_zero_followed_by_variable() {
        let mut lexer = Lexer::new("0t");
        assert_token(&mut lexer, Token::Number(0), 0, 0, 0);
        assert_token(&mut lexer, Token::Variable, 0, 1, 1);
    }
}

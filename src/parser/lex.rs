//! Simple lexer with 1-token lookahead that handles a subset of C relevant to classic bytebeat. Only intended for a single statement of 1+ expression.
//!
//! Column aware, but should not be exposed to newlines yet. TODO: That!
use std::{iter::Peekable, str::Chars};

use crate::parser::LexError;

use super::{Operator, Span, Spanned, Token};

pub struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
    // Used to create spans for tokens
    // If we enumerate chars it's not peekable anymore!
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Lexer<'a> {
        Lexer {
            chars: input.chars().peekable(),
            pos: 0,
        }
    }

    /// Advances the iterator and increments the position counter
    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    pub fn next(&mut self) -> Spanned<Token> {
        self.skip_whitespace();
        let start = self.pos;
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
                    // II: Numbers (always into i32)
                    '0'..='9' => {
                        // Python moment
                        let mut number_string = String::new();
                        number_string.push(self.bump().unwrap());
                        while let Some(&peeked) = self.chars.peek() {
                            if peeked.is_numeric() {
                                number_string.push(self.bump().unwrap());
                            } else {
                                break;
                            }
                        }
                        // Should be okay since we're already matching numerals
                        Token::Number(number_string.parse().unwrap())
                    }
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

        let end = if self.pos > start {
            self.pos - 1
        } else {
            start
        };
        Spanned::new(token, Span::new(start, end))
    }

    fn skip_whitespace(&mut self) {
        while let Some(&peeked) = self.chars.peek() {
            if peeked.is_whitespace() {
                self.bump();
            } else {
                return;
            }
        }
    }
}

// Mostly focused on verifying span positions
#[cfg(test)]
mod tests {
    use super::*;

    fn assert_token(lexer: &mut Lexer, expected_token: Token, start: usize, end: usize) {
        let spanned = lexer.next();
        assert_eq!(
            spanned.node, expected_token,
            "Token mismatch at {}-{}",
            start, end
        );
        assert_eq!(spanned.span.start, start, "Start index mismatch");
        assert_eq!(spanned.span.end, end, "End index mismatch");
    }

    // Entirely 1-char lexemes without whitespace
    #[test]
    fn test_single_char_no_whitespace() {
        let input = "t+t";
        let mut lexer = Lexer::new(input);

        // 't' at 0..1 (len 1) -> 0, 0
        assert_token(&mut lexer, Token::Variable, 0, 0);
        // '+' at 1..2 (len 1) -> 1, 1
        assert_token(&mut lexer, Token::Op(Operator::Plus), 1, 1);
        // 't' at 2..3 (len 1) -> 2, 2
        assert_token(&mut lexer, Token::Variable, 2, 2);

        assert_token(&mut lexer, Token::Eof, 3, 3);
    }

    // 1-char lexemes with whitespace
    #[test]
    fn test_single_char_with_whitespace() {
        let input = "t + t";
        let mut lexer = Lexer::new(input);

        // 't' at 0
        assert_token(&mut lexer, Token::Variable, 0, 0);
        // ' ' at 1 (skip)
        // '+' at 2
        assert_token(&mut lexer, Token::Op(Operator::Plus), 2, 2);
        // ' ' at 3 (skip)
        // 't' at 4
        assert_token(&mut lexer, Token::Variable, 4, 4);

        assert_token(&mut lexer, Token::Eof, 5, 5);
    }

    // realistic multi-char lexemes with whitespace
    #[test]
    fn test_multi_char_lexemes() {
        // "123 == 45"
        // 012 -> 123 (len 3) -> start 0, end 2
        // 3 -> space
        // 45 -> == (len 2) -> start 4, end 5
        // 6 -> space
        // 78 -> 45 (len 2) -> start 7, end 8
        let input = "123 == 45";
        let mut lexer = Lexer::new(input);

        assert_token(&mut lexer, Token::Number(123), 0, 2);
        assert_token(&mut lexer, Token::Op(Operator::Eq), 4, 5);
        assert_token(&mut lexer, Token::Number(45), 7, 8);
        assert_token(&mut lexer, Token::Eof, 9, 9);
    }

    #[test]
    fn test_eof_empty() {
        let input = "";
        let mut lexer = Lexer::new(input);
        assert_token(&mut lexer, Token::Eof, 0, 0);
    }

    #[test]
    fn test_whitespace_only() {
        let input = "   ";
        let mut lexer = Lexer::new(input);
        assert_token(&mut lexer, Token::Eof, 3, 3);
    }

    #[test]
    fn test_error_tokens() {
        let input = "=";
        let mut lexer = Lexer::new(input);
        let token = lexer.next();
        if let Token::Err(LexError::SolitaryEquals) = token.node {
        } else {
            panic!("Expected SolitaryEquals, got {:?}", token.node);
        }

        let input = "@";
        let mut lexer = Lexer::new(input);
        let token = lexer.next();
        if let Token::Err(LexError::UnexpectedChar('@')) = token.node {
        } else {
            panic!("Expected UnexpectedChar(@), got {:?}", token.node);
        }
    }
}

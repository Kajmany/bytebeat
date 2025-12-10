//! Simple lexer with 1-token lookahead that handles a subset of C relevant to classic bytebeat. Only intended for a single statement of 1+ expression.
//! LLM SLOP PRESENCE: Modest (added the rest of the tokens)
use std::{iter::Peekable, str::Chars};

use super::{Operator, Token};

pub struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Lexer<'a> {
        Lexer {
            chars: input.chars().peekable(),
        }
    }

    pub fn next(&mut self) -> Token {
        self.skip_whitespace();
        match self.chars.peek() {
            Some(&c) => {
                match c {
                    // I: Operators, including multi-char
                    '+' => {
                        self.chars.next();
                        Token::Op(Operator::Plus)
                    }
                    '-' => {
                        self.chars.next();
                        Token::Op(Operator::Minus)
                    }
                    '/' => {
                        self.chars.next();
                        Token::Op(Operator::Div)
                    }
                    '*' => {
                        self.chars.next();
                        Token::Op(Operator::Mul)
                    }
                    '%' => {
                        self.chars.next();
                        Token::Op(Operator::Mod)
                    }
                    '&' => {
                        self.chars.next();
                        if let Some('&') = self.chars.peek() {
                            self.chars.next();
                            Token::Op(Operator::LogAnd)
                        } else {
                            Token::Op(Operator::And)
                        }
                    }
                    '|' => {
                        self.chars.next();
                        if let Some('|') = self.chars.peek() {
                            self.chars.next();
                            Token::Op(Operator::LogOr)
                        } else {
                            Token::Op(Operator::Or)
                        }
                    }
                    '^' => {
                        self.chars.next();
                        Token::Op(Operator::BitXor)
                    }
                    '~' => {
                        self.chars.next();
                        Token::Op(Operator::BitNot)
                    }
                    '!' => {
                        self.chars.next();
                        if let Some('=') = self.chars.peek() {
                            self.chars.next();
                            Token::Op(Operator::Ne)
                        } else {
                            Token::Op(Operator::LogNot)
                        }
                    }
                    '=' => {
                        self.chars.next();
                        if let Some('=') = self.chars.peek() {
                            self.chars.next();
                            Token::Op(Operator::Eq)
                        } else {
                            todo!("Assignment or single = not supported")
                        }
                    }
                    '?' => {
                        self.chars.next();
                        Token::Op(Operator::Question)
                    }
                    ':' => {
                        self.chars.next();
                        Token::Op(Operator::Colon)
                    }
                    '(' => {
                        self.chars.next();
                        Token::Op(Operator::Lparen)
                    }
                    ')' => {
                        self.chars.next();
                        Token::Op(Operator::Rparen)
                    }
                    '<' => {
                        self.chars.next(); // consume first <
                        if let Some(&next) = self.chars.peek() {
                            if next == '<' {
                                self.chars.next();
                                Token::Op(Operator::Lsh)
                            } else if next == '=' {
                                self.chars.next();
                                Token::Op(Operator::Le)
                            } else {
                                Token::Op(Operator::Lt)
                            }
                        } else {
                            Token::Op(Operator::Lt)
                        }
                    }
                    '>' => {
                        self.chars.next();
                        if let Some(&next) = self.chars.peek() {
                            if next == '>' {
                                self.chars.next();
                                Token::Op(Operator::Rsh)
                            } else if next == '=' {
                                self.chars.next();
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
                        number_string.push(self.chars.next().unwrap());
                        while let Some(&peeked) = self.chars.peek() {
                            if peeked.is_numeric() {
                                number_string.push(self.chars.next().unwrap());
                            } else {
                                break;
                            }
                        }
                        // Should be okay since we're already matching numerals
                        Token::Number(number_string.parse().unwrap())
                    }
                    // III: Variables. Could be anything, but we restrict to 't' for the users' sanity.
                    't' => {
                        self.chars.next();
                        Token::Variable
                    }
                    _ => {
                        // TODO: We want to keep going and just stack up errors
                        todo!("Unexpected character: {}", self.chars.next().unwrap())
                    }
                }
            }
            // IV: End
            None => Token::Eof,
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(&peeked) = self.chars.peek() {
            if peeked.is_whitespace() {
                self.chars.next();
            } else {
                return;
            }
        }
    }
}

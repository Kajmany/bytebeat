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
                            // Assignment not supported, or treat as error?
                            // Lexer usually returns error or unexpected token?
                            // User asked for ==.
                            // Treat single = as unknown or error?
                            // Current lexer panics or todo on unknown?
                            // `_` arm handles atoms.
                            // Lexer structure matches specific chars.
                            // If I don't match `=`, it goes to `_` -> variable starting with `=`?
                            // `Lexer::reserved_char` includes operators. I should add `=` to reserved chars.
                            // But for now, if I match `=`, I expect `==`.
                            // If not `==`, panic/todo?
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
                    // II: Atoms: numbers and variables
                    // Variables may not start with numbers, like C
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
                        Token::Atom(number_string)
                    }
                    // Unlike C, we'll try to make anything that's not already matched a variable.
                    // TODO: was this regrettable?
                    _ => {
                        let mut variable_string = String::new();
                        // Match should be desirable since whitespace, numeric, reserved handled already
                        variable_string.push(self.chars.next().unwrap());
                        while let Some(&peeked) = self.chars.peek() {
                            if peeked.is_whitespace()
                                | peeked.is_numeric()
                                | Lexer::reserved_char(peeked)
                            {
                                break;
                            } else {
                                variable_string.push(self.chars.next().unwrap());
                            }
                        }
                        Token::Atom(variable_string)
                    }
                }
            }
            // III: End
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

    /// Variables may not contain any operator characters anywhere inside
    fn reserved_char(c: char) -> bool {
        matches!(
            c,
            '+' | '-'
                | '/'
                | '*'
                | '%'
                | '&'
                | '|'
                | '^'
                | '!'
                | '='
                | '~'
                | '?'
                | ':'
                | '('
                | ')'
                | '<'
                | '>'
        )
    }
}

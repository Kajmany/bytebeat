use std::{iter::Peekable, str::Chars};

enum Token {
    // TODO: split atom to number and variable
    Atom(String),
    Op(Operator),
    Eof,
}

enum Operator {
    Rsh,
    Lsh,
    Mul,
    Div,
    Plus,
    Minus,
    Mod,
    Lparen,
    Rparen,
    // Bitwise
    And,
    Or,
    // Ternary operator
    Question,
    Colon,
}

struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Lexer<'a> {
        Lexer {
            chars: input.chars().peekable(),
        }
    }

    fn next(&mut self) -> Token {
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
                        Token::Op(Operator::And)
                    }
                    '|' => {
                        self.chars.next();
                        Token::Op(Operator::Or)
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
                        let cur = self.chars.next().unwrap();
                        if let Some(next) = self.chars.peek() {
                            if cur == '<' && *next == '<' {
                                self.chars.next();
                                Token::Op(Operator::Lsh)
                            } else {
                                // TODO: error handling
                                todo!()
                            }
                        } else {
                            // TODO: error handling
                            todo!()
                        }
                    }
                    '>' => {
                        let cur = self.chars.next().unwrap();
                        if let Some(next) = self.chars.peek() {
                            if cur == '>' && *next == '>' {
                                self.chars.next();
                                Token::Op(Operator::Rsh)
                            } else {
                                // TODO: error handling
                                todo!()
                            }
                        } else {
                            // TODO: error handling
                            todo!()
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
            '+' | '-' | '/' | '*' | '%' | '&' | '|' | '?' | ':' | '(' | ')' | '<' | '>'
        )
    }
}

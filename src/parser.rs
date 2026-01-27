//! Converts [`String`] input to functions that evaluate into classic (i32 -> u8) bytebeat, or accrues a
//! vec full of errors while trying.
//!
//! LLM SLOP PRESENCE: EXTREME
pub mod lex;
pub mod parse;

use std::fmt;
use std::ops::Deref;

use self::parse::Parser;

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    /// Must be 't'
    Variable,
    Number(i32),
    Op(Operator),
    /// Represents an lexer-specific error. Not directly parsable.
    // Is this a smart way to do lazy errors, or a hack? Both?
    Err(LexError),
    Eof,
}

pub type Column = usize;
pub type Line = usize;

/// Represents the start and end occurence of a [`Token`] in the source buffer
/// Inclusive on both ends, so a span of [0, 0, 0] is a single character at the start.
///
/// No [`Token`] members may span multiple lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: Line,
    pub start: Column,
    pub end: Column,
}

impl Span {
    pub fn new(line: Line, start: Column, end: Column) -> Self {
        Self { line, start, end }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start == self.end {
            write!(f, "line {} col {}", self.line, self.start)
        } else {
            write!(f, "line {} col {}..{}", self.line, self.start, self.end)
        }
    }
}

/// Every token is wrapped in a [`Span`] using this.
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}

impl<T> Deref for Spanned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.node
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Operator {
    Mul,
    Div,
    Plus,
    Minus,
    Mod,
    Lparen,
    Rparen,
    // Bitwise
    Rsh,
    Lsh,
    And,
    Or,
    BitXor,
    BitNot,
    // Logical
    LogAnd,
    LogOr,
    LogNot,
    // Relational
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    // Ternary operator
    /// Part of the ternary operator.
    Question,
    /// Part of the ternary operator.
    Colon,
}

pub type NodeId = usize;

#[derive(Debug, PartialEq, Clone)]
pub enum ASTNode {
    Literal(i32),
    Variable,
    Binary(Operator, NodeId, NodeId),
    Ternary(NodeId, NodeId, NodeId),
    /// Because [`Beat`] uses these too, we're making invalid state representable.
    /// there's logic elsewhere that should prevent creation of a valid beat with these.
    Error(Span),
}

use thiserror::Error;
use tracing::error;

/// Span IS attached because these are not wrapped and meant to be returned outside module
#[derive(Error, Debug, PartialEq)]
pub enum ParseError {
    #[error("Unexpected end of file at {0}")]
    UnexpectedEof(Span),
    // TODO: not pretty that we debug fmt this (it's user facing)
    // but I don't feel like impl fmt for every token right now
    #[error("Expected operator, found {0:?} at {1}")]
    ExpectedOperator(Token, Span),
    #[error("Expected matching ')' at {0}")]
    UnmatchedParenthesis(Span),
    #[error("Unexpected prefix operator: {0:?} at {1}")]
    UnexpectedPrefix(Operator, Span),
    #[error("Expected ':' in ternary expression at {0}")]
    ExpectedTernaryColon(Span),
    #[error("Lexer: {0} at {1}")]
    LexError(LexError, Span),
}

/// Span is NOT attached because these errors are either in a [`Token::Err`]`
/// or in a [`ParseError::LexError`] which carries the relevant [`Span`]
#[derive(Error, Debug, PartialEq, Clone)]
pub enum LexError {
    #[error("Expected valid base {0} number: {1}")]
    ImproperNumber(i8, std::num::ParseIntError),
    #[error("Assignment or single = not supported")]
    SolitaryEquals,
    #[error("Unexpected character: {0}")]
    UnexpectedChar(char),
}

#[derive(Debug, Default)]
/// AST of a classic bytebeat function. May be evaluated for 't' into a u8 sample. Can be empty, and produce no sound.
pub struct Beat {
    // Could be a real arena but not practically necessary
    nodes: Vec<ASTNode>,
    root: NodeId,
}

impl Beat {
    /// Attempt to turn a string into an evaluable beat. Empty strings produce silent beats.
    pub fn compile(source: &str) -> Result<Beat, Vec<ParseError>> {
        if source.is_empty() {
            Ok(Beat::default())
        } else {
            let mut nodes = Vec::new();
            let root = Parser::new(source, &mut nodes).parse()?;
            Ok(Beat { nodes, root })
        }
    }

    pub fn eval(&self, t: i32) -> u8 {
        if self.nodes.is_empty() {
            0
        } else {
            self.eval_node(self.root, t) as u8
        }
    }

    fn eval_node(&self, id: NodeId, t: i32) -> i32 {
        match &self.nodes[id] {
            ASTNode::Literal(n) => *n,
            ASTNode::Variable => t,
            ASTNode::Binary(op, left, right) => {
                let l = self.eval_node(*left, t);
                let r = self.eval_node(*right, t);
                match op {
                    Operator::Plus => l.wrapping_add(r),
                    Operator::Minus => l.wrapping_sub(r),
                    Operator::Mul => l.wrapping_mul(r),
                    Operator::Div => {
                        if r == 0 {
                            0
                        } else {
                            l.wrapping_div(r)
                        }
                    }
                    Operator::Mod => {
                        if r == 0 {
                            0
                        } else {
                            l.wrapping_rem(r)
                        }
                    }
                    Operator::And => l & r,
                    Operator::Or => l | r,
                    Operator::BitXor => l ^ r,
                    Operator::Lsh => l.wrapping_shl(r as u32),
                    Operator::Rsh => l.wrapping_shr(r as u32),
                    Operator::LogAnd => {
                        if l != 0 && r != 0 {
                            1
                        } else {
                            0
                        }
                    }
                    Operator::LogOr => {
                        if l != 0 || r != 0 {
                            1
                        } else {
                            0
                        }
                    }
                    Operator::Eq => {
                        if l == r {
                            1
                        } else {
                            0
                        }
                    }
                    Operator::Ne => {
                        if l != r {
                            1
                        } else {
                            0
                        }
                    }
                    Operator::Gt => {
                        if l > r {
                            1
                        } else {
                            0
                        }
                    }
                    Operator::Lt => {
                        if l < r {
                            1
                        } else {
                            0
                        }
                    }
                    Operator::Ge => {
                        if l >= r {
                            1
                        } else {
                            0
                        }
                    }
                    Operator::Le => {
                        if l <= r {
                            1
                        } else {
                            0
                        }
                    }
                    Operator::BitNot => !r,
                    Operator::LogNot => {
                        if r == 0 {
                            1
                        } else {
                            0
                        }
                    }
                    _ => 0,
                }
            }
            ASTNode::Ternary(cond, true_branch, false_branch) => {
                let c = self.eval_node(*cond, t);
                if c != 0 {
                    self.eval_node(*true_branch, t)
                } else {
                    self.eval_node(*false_branch, t)
                }
            }
            // This shouldn't ever happen!
            ASTNode::Error(_) => {
                error!("Beat is evaluating an AST that has error nodes. This is a program bug!");
                0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Arbitrary. Many songs have longer periodicity. We can probably call it good here, though.
    const SAMPLES_TO_COMPARE: i32 = 2_i32.pow(16);
    // Hard-coded jump-table entry point. Build.rs puts together the songs in the same order of the static array
    #[link(name = "parity_dispatcher")]
    // Lots of beats will crash our test runner with a not-useful message of what was responsible.
    // The shell script actually runs these to try to screen them out
    unsafe extern "C" {
        fn generate_sample(song_idx: i32, t: i32) -> u8;
    }

    fn compare_song(song_idx: i32, beat: &str) {
        let ours = Beat::compile(beat).expect("expected beat to compile in our parser");
        for t in 0..SAMPLES_TO_COMPARE {
            assert_eq!(unsafe { generate_sample(song_idx, t) }, ours.eval(t))
        }
    }

    include!(concat!(env!("OUT_DIR"), "/parity_tests.rs"));
}

//! Converts [`String`] input to functions that evaluate into classic (i32 -> u8) bytebeat, or dies trying.
//! LLM SLOP PRESENCE: EXTREME
pub mod lex;
pub mod parse;

use self::parse::Parser;

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    // TODO: split atom to number and variable
    Atom(String),
    Op(Operator),
    Eof,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Operator {
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

pub type NodeId = usize;

#[derive(Debug, PartialEq, Clone)]
pub enum ASTNode {
    Literal(i32),
    Variable(String),
    Binary(Operator, NodeId, NodeId),
    Ternary(NodeId, NodeId, NodeId),
}

use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ParseError {
    #[error("Unexpected end of file")]
    UnexpectedEof,
    #[error("Expected operator, found something else")]
    ExpectedOperator,
    #[error("Expected matching ')'")]
    UnmatchedParenthesis,
    #[error("Unexpected prefix operator: {0:?}")]
    UnexpectedPrefix(Operator),
    #[error("Expected ':' in ternary expression")]
    ExpectedTernaryColon,
}

pub struct Beat {
    nodes: Vec<ASTNode>,
    root: NodeId,
}

impl Beat {
    pub fn compile(source: &str) -> Result<Beat, ParseError> {
        let mut nodes = Vec::new();
        let root = Parser::new(source, &mut nodes).parse()?;
        Ok(Beat { nodes, root })
    }

    pub fn eval(&self, t: i32) -> u8 {
        self.eval_node(self.root, t) as u8
    }

    fn eval_node(&self, id: NodeId, t: i32) -> i32 {
        match &self.nodes[id] {
            ASTNode::Literal(n) => *n,
            ASTNode::Variable(_) => t, // TODO: Enforce this only at the front end or make it more clear inside this code we only do one var
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
                    Operator::Lsh => l.wrapping_shl(r as u32),
                    Operator::Rsh => l.wrapping_shr(r as u32),
                    _ => 0, // Should not happen for valid binary ops in this set
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_program_eval_basic() {
        let prog = Beat::compile("t + 1").unwrap();
        assert_eq!(prog.eval(10), 11);
    }

    #[test]
    fn test_program_eval_precedence() {
        let prog = Beat::compile("t * 2 + 1").unwrap();
        assert_eq!(prog.eval(10), 21);

        let prog2 = Beat::compile("t * (2 + 1)").unwrap();
        assert_eq!(prog2.eval(10), 30);
    }

    #[test]
    fn test_ternary() {
        // if t > 5 (t-5 > 0 ?) ...
        // bytebeat often uses `t` as bool.
        // 0 is false, non-zero is true.
        let prog = Beat::compile("t ? 100 : 200").unwrap();
        assert_eq!(prog.eval(1), 100);
        assert_eq!(prog.eval(0), 200);
    }

    #[test]
    fn test_program_eval_bitwise() {
        let prog = Beat::compile("t >> 1").unwrap();
        assert_eq!(prog.eval(256), 128);
    }

    // Test more advanced examples of actual bytebeat
    // TODO: maybe externally verify with an actual C compiler or research Rust quirks here

    #[test]
    fn test_42_melody_parity() {
        // The periodicity appears to be 2^16 so we'll do it all to be safe
        let expected: Vec<u8> = (0..65536).map(|t| (t * (42 & (t >> 10))) as u8).collect();
        let prog = Beat::compile("t*(42&t>>10)").unwrap();
        let actual: Vec<u8> = (0..65536).map(|t| prog.eval(t)).collect();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_neurofunk_parity() {
        // This one is much higher but let's not do u32::MAX iterations, okay?
        // This was a nightmare TODO: check the slop translation of this again
        let expected: Vec<u8> = (0..65536)
            .map(|t| {
                (t as i32
                    * ((if t & 4096 != 0 {
                        if t % 65536 < 59392 { 7 } else { t & 7 }
                    } else {
                        16
                    }) + (1 & (t >> 14))))
                    >> ((3 & (-t >> (if t & 2048 != 0 { 2 } else { 10 }))) as i32)
                    | (t >> (if t & 16384 != 0 {
                        if t & 4096 != 0 { 10 } else { 3 }
                    } else {
                        2
                    }))
            } as u8)
            .collect();
        let prog = Beat::compile("t*((t&4096?t%65536<59392?7:t&7:16)+(1&t>>14))>>(3&-t>>(t&2048?2:10))|t>>(t&16384?t&4096?10:3:2)").unwrap();
        let actual: Vec<u8> = (0..65536).map(|t| prog.eval(t)).collect();
        assert_eq!(expected, actual);
    }
}

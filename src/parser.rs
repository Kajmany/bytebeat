//! Converts [`String`] input to functions that evaluate into classic (i32 -> u8) bytebeat, or accrues a
//! vec full of errors while trying.
//!
//! LLM SLOP PRESENCE: EXTREME
pub mod lex;
pub mod parse;

use std::fmt;
use std::ops::Deref;

use self::parse::Parser;

// Only an error path does a copy
#[derive(Debug, PartialEq, Clone, Copy)]
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
#[derive(Error, Debug, PartialEq, Clone, Copy)]
pub enum LexError {
    #[error("Assignment or single = not supported")]
    SolitaryEquals,
    #[error("Unexpected character: {0}")]
    UnexpectedChar(char),
}

#[derive(Debug)]
/// AST of a classic bytebeat function. May be evaluated for 't' into a u8 sample.
pub struct Beat {
    // Could be a real arena but not practically necessary
    nodes: Vec<ASTNode>,
    root: NodeId,
}

impl Beat {
    pub fn compile(source: &str) -> Result<Beat, Vec<ParseError>> {
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
    use std::io::Read;
    use std::path::Path;
    use std::process::Command;
    use std::sync::Once;

    // FIXME: I'm willing to give this nonsense something of a pass because it works and it's just for testing
    // but this does not seem ideal for portability, politeness, security, etc. etc.
    // and it's all EXTREMELY `AI`
    static INIT: Once = Once::new();
    fn ensure_references(filename: &str) -> Option<Vec<u8>> {
        let bin_path_str = format!("target/{}", filename);
        let bin_path = Path::new(&bin_path_str);

        // Try to generate if missing (lazy generation of ALL files at once)
        if !bin_path.exists() {
            INIT.call_once(|| {
                // Ensure target directory exists
                let _ = std::fs::create_dir_all("target");

                // Check for gcc or clang
                let mut compiler = "gcc";
                if Command::new(compiler).arg("--version").output().is_err() {
                    compiler = "clang";
                    if Command::new(compiler).arg("--version").output().is_err() {
                        eprintln!("Neither gcc nor clang found. Skipping generation.");
                        return;
                    }
                }

                // Compile
                let src_path = "src/parser/generate_references.c";
                if !Path::new(src_path).exists() {
                    eprintln!("C source not found at {}", src_path);
                    return;
                }

                // Compile to a temporary executable
                let compile_status = Command::new(compiler)
                    .arg(src_path)
                    .arg("-o")
                    .arg("target/generate_references")
                    .status();

                if let Ok(status) = compile_status {
                    if status.success() {
                        // Run generator, which produces files in current directory (target)
                        // We run it with current_dir = target so files land there
                        let _ = Command::new("./generate_references")
                            .current_dir("target")
                            .status();
                    }
                }
            });
        }

        // Read file
        if let Ok(mut file) = std::fs::File::open(bin_path) {
            let mut buffer = Vec::new();
            if file.read_to_end(&mut buffer).is_ok() {
                return Some(buffer);
            }
        }
        // Assuming success if file exists now.
        None
    }

    fn check_parity(reference_file: &str, code: &str) {
        let refs = ensure_references(reference_file);
        if refs.is_none() {
            eprintln!(
                "Skipping test for '{}' - reference file '{}' missing.",
                code, reference_file
            );
            return;
        }
        let refs = refs.unwrap();

        // Expected size
        if refs.len() < 65536 {
            panic!(
                "Reference file {} too small! Expected 65536 bytes, got {}",
                reference_file,
                refs.len()
            );
        }

        let expected = &refs[..65536];
        let prog = Beat::compile(code).expect("Failed to compile bytebeat");

        for t in 0..65536 {
            let val = prog.eval(t);
            if val != expected[t as usize] {
                panic!(
                    "Mismatch at t={}: expected {}, got {}. Code: {}",
                    t, expected[t as usize], val, code
                );
            }
        }
    }

    #[test]
    fn test_42_melody_parity() {
        // "the 42 melody" (Community)
        check_parity("reference_42_melody.bin", "t*(42&t>>10)");
    }

    #[test]
    fn test_neurofunk_parity() {
        // "Neurofunk" by SthephanShi
        // Code covers: *, &, ?, %, <, +, >>, -, |
        check_parity(
            "reference_neurofunk.bin",
            "t*((t&4096?t%65536<59392?7:t&7:16)+(1&t>>14))>>(3&-t>>(t&2048?2:10))|t>>(t&16384?t&4096?10:3:2)",
        );
    }

    #[test]
    fn test_chip_parity() {
        // "chip" by Butterroach
        // Code covers: ||, &&, !, ~, ? :
        check_parity(
            "reference_chip.bin",
            "(t&1024||t&16384&&t&2048&&!(t&512))?(t&4096&&!(t&2048)?(t*t*t>>~t*t)+127:t*((t>>11&1)+1)*(1+(t>>16&1)*3))*2:0",
        );
    }

    #[test]
    fn test_bytebreak_parity() {
        // "Bytebreak" by WoolWL
        // Code covers: ==, !=, ^, /
        check_parity(
            "reference_bytebreak.bin",
            "((t&32767)>>13==2|(t&65535)>>12==9?(t^-(t/8&t>>5)*(t/8&127))&(-(t>>5)&255)*((t&65535)>>12==9?2:1):(t&8191)%((t>>5&255^240)==0?1:t>>5&255^240))/4*3+(t*4/(4+(t>>15&3))&128)*(-t>>11&2)*((t&32767)>>13!=2)/3",
        );
    }

    #[test]
    fn test_wheezing_modem_parity() {
        // "Wheezing modem" by SthephanShi
        // Code covers: <<
        check_parity(
            "reference_wheezing_modem.bin",
            "100*((t<<2|t>>5|t^63)&(t<<10|t>>11))",
        );
    }

    #[test]
    fn test_electrohouse_parity() {
        // "Electrohouse" by Anonymous (from Russian imageboards)
        // Code covers: >=
        check_parity(
            "reference_electrohouse.bin",
            "t>>(((t%2?t%((t>>13)%8>=2?((t>>13)%8>=4?41:51):61):t%34)))|(~t>>4)",
        );
    }

    #[test]
    fn test_hit_of_the_season_parity() {
        // "THE HIT OF THE SEASON" by Anonymous (from Russian imageboards)
        // Code covers: >
        check_parity(
            "reference_hit_of_the_season.bin",
            "(t>0&t<65535?t%32>(t/10000)?t>>4:t>>6:0)&(t>>4)",
        );
    }
}

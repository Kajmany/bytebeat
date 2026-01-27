//! Pratt Parser intended to handle a single statement in a C subset.
//!
//! Will wrap multiple errors (including Lexer errors) and has some (unreliable) recoverability.
//!
//! LLM SLOP PRESENCE: EXTREME
use crate::parser::Spanned;

use super::lex::Lexer;
use super::{ASTNode, NodeId, Operator, ParseError, Token};

/// Wraps a lexer and pulls tokens out to build an AST. Must process a single statement with at least one expression.
pub struct Parser<'a, 'b> {
    lexer: Lexer<'a>,
    current: Spanned<Token>,
    arena: &'b mut Vec<ASTNode>,
    errors: Vec<ParseError>,
}

impl<'a, 'b> Parser<'a, 'b> {
    pub fn new(input: &'a str, arena: &'b mut Vec<ASTNode>) -> Self {
        let mut lexer = Lexer::new(input);
        let current = lexer.next();
        Parser {
            lexer,
            current,
            arena,
            errors: Vec::new(),
        }
    }

    fn advance(&mut self) {
        self.current = self.lexer.next();
    }

    pub fn parse(&mut self) -> Result<NodeId, Vec<ParseError>> {
        let result = self.parse_bp(0);
        match result {
            // We may still have errors overall even if the root parse is Ok.
            Ok(root) => {
                if self.errors.is_empty() {
                    Ok(root)
                } else {
                    Err(std::mem::take(&mut self.errors))
                }
            }
            Err(e) => {
                self.errors.push(e);
                Err(std::mem::take(&mut self.errors))
            }
        }
    }

    fn push_node(&mut self, node: ASTNode) -> NodeId {
        let id = self.arena.len();
        self.arena.push(node);
        id
    }

    fn parse_bp(&mut self, min_bp: u8) -> Result<NodeId, ParseError> {
        let mut left = match *self.current {
            Token::Number(n) => {
                let node = ASTNode::Literal(n);
                self.advance();
                self.push_node(node)
            }
            Token::Variable => {
                let node = ASTNode::Variable;
                self.advance();
                self.push_node(node)
            }
            Token::Op(Operator::Lparen) => {
                self.advance();
                let expr = self.parse_bp(0)?;
                if let Token::Op(Operator::Rparen) = *self.current {
                    self.advance();
                    expr
                } else {
                    return Err(ParseError::UnmatchedParenthesis(self.current.span));
                }
            }
            Token::Op(op) => {
                // Prefix operators handling (Unary minus, etc.)
                let (_, right_bp) = match op {
                    Operator::Minus | Operator::Plus | Operator::LogNot | Operator::BitNot => {
                        ((), 99)
                    }
                    _ => return Err(ParseError::UnexpectedPrefix(op, self.current.span)),
                };

                // Need to consume the operator
                let op_val = op;
                self.advance();
                let right = self.parse_bp(right_bp)?;

                match op_val {
                    Operator::Minus => {
                        let zero = self.push_node(ASTNode::Literal(0));
                        self.push_node(ASTNode::Binary(Operator::Minus, zero, right))
                    }
                    Operator::Plus => right,
                    Operator::LogNot => {
                        let zero = self.push_node(ASTNode::Literal(0));
                        self.push_node(ASTNode::Binary(Operator::LogNot, zero, right))
                    }
                    Operator::BitNot => {
                        let zero = self.push_node(ASTNode::Literal(0));
                        self.push_node(ASTNode::Binary(Operator::BitNot, zero, right))
                    }
                    _ => unreachable!(),
                }
            }
            Token::Err(ref e) => {
                let span = self.current.span;
                let err = ParseError::LexError(e.clone(), span);
                self.errors.push(err);
                self.advance();
                self.push_node(ASTNode::Error(span))
            }
            Token::Eof => return Err(ParseError::UnexpectedEof(self.current.span)),
        };

        loop {
            let op = match *self.current {
                Token::Op(op) => op,
                Token::Eof => break,
                Token::Err(ref e) => {
                    let span = self.current.span;
                    let err = ParseError::LexError(e.clone(), span);
                    self.errors.push(err);
                    self.advance();
                    // TODO: This probably leaves a lot of gaps but not high priority
                    // this is best-effort to getting all errors we can at once
                    break;
                }
                ref t => {
                    // Need to clone state on this path to send a rich error
                    return Err(ParseError::ExpectedOperator(
                        t.clone(),
                        self.current.span.clone(),
                    ));
                }
            };

            // Postfix ?
            if let Operator::Question = op {
                let (l_bp, r_bp) = infix_binding_power(op);
                if l_bp < min_bp {
                    break;
                }
                self.advance(); // consume '?'

                let true_branch = self.parse_bp(0)?;

                if let Token::Op(Operator::Colon) = *self.current {
                    self.advance(); // consume ':'
                    let false_branch = self.parse_bp(r_bp)?;
                    left = self.push_node(ASTNode::Ternary(left, true_branch, false_branch));
                    continue;
                } else {
                    return Err(ParseError::ExpectedTernaryColon(self.current.span));
                }
            }

            if let Some((l_bp, r_bp)) = binding_power(op) {
                if l_bp < min_bp {
                    break;
                }

                self.advance();
                let right = self.parse_bp(r_bp)?;
                left = self.push_node(ASTNode::Binary(op, left, right));
                continue;
            } else {
                // Not an infix operator (e.g. Rparen) or unknown
                break;
            }
        }

        Ok(left)
    }
}

fn binding_power(op: Operator) -> Option<(u8, u8)> {
    match op {
        // Multiplicative
        Operator::Mul | Operator::Div | Operator::Mod => Some((80, 81)),
        // Additive
        Operator::Plus | Operator::Minus => Some((70, 71)),
        // Shifts
        Operator::Lsh | Operator::Rsh => Some((60, 61)),
        // Relational
        Operator::Lt | Operator::Gt | Operator::Le | Operator::Ge => Some((50, 51)),
        // Equality
        Operator::Eq | Operator::Ne => Some((45, 46)),
        // Bitwise
        Operator::And => Some((40, 41)),
        Operator::BitXor => Some((35, 36)),
        Operator::Or => Some((30, 31)),
        // Logical
        Operator::LogAnd => Some((25, 26)),
        Operator::LogOr => Some((20, 21)),
        _ => None,
    }
}

fn infix_binding_power(op: Operator) -> (u8, u8) {
    match op {
        Operator::Question => (10, 9), // Right associative?
        _ => (0, 0),
    }
}

// None of these slopped tests are terribly useful (since Beat's tests are more realistic)
// But the recovery is interesting to ensure we have a temporarily sensible AST still.
#[cfg(test)]
mod tests {
    use crate::parser::LexError;

    use super::*;

    #[test]
    fn test_basic_arithmetic() {
        let mut arena = Vec::new();
        let mut p = Parser::new("1 + 2 * 3", &mut arena);
        let root = p.parse().unwrap();

        // 1 + (2 * 3)
        // Root should be Binary(Plus, 1, Binary(Mul, 2, 3))
        if let ASTNode::Binary(Operator::Plus, l_id, r_id) = &arena[root] {
            assert_eq!(arena[*l_id], ASTNode::Literal(1));
            if let ASTNode::Binary(Operator::Mul, rl_id, rr_id) = &arena[*r_id] {
                assert_eq!(arena[*rl_id], ASTNode::Literal(2));
                assert_eq!(arena[*rr_id], ASTNode::Literal(3));
            } else {
                panic!("Right side structure wrong");
            }
        } else {
            panic!("Top structure wrong");
        }
    }

    #[test]
    fn test_recovery() {
        let mut arena = Vec::new();
        let mut p = Parser::new("t + @", &mut arena);
        let result = p.parse();

        match result {
            Ok(_) => panic!("Should have returned error"),
            Err(errors) => {
                assert_eq!(errors.len(), 1);
                if let ParseError::LexError(LexError::UnexpectedChar('@'), _) = errors[0] {
                } else {
                    panic!("Wrong error type: {:?}", errors[0]);
                }
            }
        }

        assert_eq!(arena.len(), 3);
        assert!(matches!(arena[1], ASTNode::Error(_)));
        assert!(matches!(arena[2], ASTNode::Binary(Operator::Plus, _, _)));
    }

    #[test]
    fn test_multiple_errors() {
        let mut arena = Vec::new();
        let mut p = Parser::new("@ + @", &mut arena);
        let result = p.parse();

        match result {
            Ok(_) => panic!("Should have returned errors"),
            Err(errors) => {
                assert_eq!(errors.len(), 2);
            }
        }
    }

    #[test]
    fn test_ternary_precedence() {
        let mut arena = Vec::new();
        // t > 128 ? t : 0
        // > is (50, 51). ? is (10, 9).
        let mut p = Parser::new("t > 128 ? t : 0", &mut arena);
        let root = p.parse().unwrap();

        if let ASTNode::Ternary(cond, _, _) = &arena[root] {
            assert!(matches!(arena[*cond], ASTNode::Binary(Operator::Gt, _, _)));
        } else {
            panic!("Precedence check failed given: {:?}", arena[root]);
        }
    }

    #[test]
    fn test_ternary_nested() {
        let mut arena = Vec::new();
        // t ? t ? 1 : 2 : 0
        // Should parse as t ? (t ? 1 : 2) : 0 because of right associativity (10, 9)
        let mut p = Parser::new("t ? t ? 1 : 2 : 0", &mut arena);
        let root = p.parse().unwrap();

        if let ASTNode::Ternary(cond, true_branch, false_branch) = &arena[root] {
            assert_eq!(arena[*cond], ASTNode::Variable);
            assert_eq!(arena[*false_branch], ASTNode::Literal(0));

            if let ASTNode::Ternary(c2, t2, f2) = &arena[*true_branch] {
                assert_eq!(arena[*c2], ASTNode::Variable);
                assert_eq!(arena[*t2], ASTNode::Literal(1));
                assert_eq!(arena[*f2], ASTNode::Literal(2));
            } else {
                panic!("Inner ternary wrong: {:?}", arena[*true_branch]);
            }
        } else {
            panic!("Top structure wrong: {:?}", arena[root]);
        }
    }

    #[test]
    fn test_ternary_recovery() {
        let mut arena = Vec::new();
        // t ? @ : @
        // Should produce 2 errors and still form a Ternary node
        let mut p = Parser::new("t ? @ : @", &mut arena);
        let result = p.parse();

        match result {
            Ok(_) => panic!("Should have returned errors"),
            Err(errors) => {
                assert_eq!(errors.len(), 2);
                assert!(matches!(errors[0], ParseError::LexError(_, _)));
                assert!(matches!(errors[1], ParseError::LexError(_, _)));
            }
        }

        // Arena should still have the structure
        assert!(arena.len() >= 4);
        let root = arena.len() - 1;
        assert!(matches!(arena[root], ASTNode::Ternary(_, _, _)));
    }

    #[test]
    fn test_recovery_in_parens() {
        let mut arena = Vec::new();
        // (@ + 1) * t
        let mut p = Parser::new("(@ + 1) * t", &mut arena);
        match p.parse() {
            Ok(_) => panic!("Should have returned errors"),
            Err(errors) => {
                assert_eq!(errors.len(), 1);
            }
        }

        let root = arena.len() - 1;
        if let ASTNode::Binary(Operator::Mul, l, r) = &arena[root] {
            assert_eq!(arena[*r], ASTNode::Variable);
            if let ASTNode::Binary(Operator::Plus, ll, lr) = &arena[*l] {
                assert!(matches!(arena[*ll], ASTNode::Error(_)));
                assert_eq!(arena[*lr], ASTNode::Literal(1));
            } else {
                panic!("Inside parens structure wrong");
            }
        } else {
            panic!("Root structure wrong");
        }
    }
}

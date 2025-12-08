//! Pratt-flavored(?) Parser intended to handle a single statement in a C subset.
//! LLM SLOP PRESENCE: EXTREME
use super::lex::Lexer;
use super::{ASTNode, NodeId, Operator, ParseError, Token};

pub struct Parser<'a, 'b> {
    lexer: Lexer<'a>,
    current_token: Token,
    arena: &'b mut Vec<ASTNode>,
}

impl<'a, 'b> Parser<'a, 'b> {
    pub fn new(input: &'a str, arena: &'b mut Vec<ASTNode>) -> Self {
        let mut lexer = Lexer::new(input);
        let current_token = lexer.next();
        Parser {
            lexer,
            current_token,
            arena,
        }
    }

    fn advance(&mut self) {
        self.current_token = self.lexer.next();
    }

    pub fn parse(&mut self) -> Result<NodeId, ParseError> {
        self.parse_bp(0)
    }

    fn push_node(&mut self, node: ASTNode) -> NodeId {
        let id = self.arena.len();
        self.arena.push(node);
        id
    }

    fn parse_bp(&mut self, min_bp: u8) -> Result<NodeId, ParseError> {
        let mut left = match &self.current_token {
            Token::Atom(s) => {
                let node = if let Ok(n) = s.parse::<i32>() {
                    ASTNode::Literal(n)
                } else {
                    ASTNode::Variable(s.clone())
                };
                self.advance();
                self.push_node(node)
            }
            Token::Op(Operator::Lparen) => {
                self.advance();
                let expr = self.parse_bp(0)?;
                if let Token::Op(Operator::Rparen) = self.current_token {
                    self.advance();
                    expr
                } else {
                    return Err(ParseError::UnmatchedParenthesis);
                }
            }
            Token::Op(op) => {
                // Prefix operators handling (Unary minus, etc.)
                let (_, right_bp) = match op {
                    Operator::Minus | Operator::Plus | Operator::LogNot | Operator::BitNot => {
                        ((), 99)
                    }
                    _ => return Err(ParseError::UnexpectedPrefix(*op)),
                };

                // Need to consume the operator
                let op_val = *op;
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
            Token::Eof => return Err(ParseError::UnexpectedEof),
        };

        loop {
            let op = match self.current_token {
                Token::Op(op) => op,
                Token::Eof => break,
                _ => return Err(ParseError::ExpectedOperator),
            };

            // Postfix ?
            if let Operator::Question = op {
                let (l_bp, r_bp) = infix_binding_power(op);
                if l_bp < min_bp {
                    break;
                }
                self.advance(); // consume '?'

                let true_branch = self.parse_bp(0)?;

                if let Token::Op(Operator::Colon) = self.current_token {
                    self.advance(); // consume ':'
                    let false_branch = self.parse_bp(r_bp)?;
                    left = self.push_node(ASTNode::Ternary(left, true_branch, false_branch));
                    continue;
                } else {
                    return Err(ParseError::ExpectedTernaryColon);
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

#[cfg(test)]
mod tests {
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
}

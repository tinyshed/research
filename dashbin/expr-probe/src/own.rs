//! A formula of ours: numbers, names, `+ - * /`, comparisons, `&& || !` and parentheses, parsed once into a tree
//! whose names are places in a row. It can only compute: there is no assignment, no call and no loop to write.
//! A value that is not there is `None` and stays so through whatever it touches: a name with no value in a row, a
//! division by zero, a number where a truth is asked.

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    /// A name, as its place among the names the formula was parsed with.
    Name(usize),
    Not(Box<Expr>),
    Negative(Box<Expr>),
    Binary(Op, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    Eq,
    NotEq,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Number(f64),
    Truth(bool),
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Name(String),
    Op(&'static str),
}

const OPS: [&str; 16] = ["&&", "||", "<=", ">=", "==", "!=", "<", ">", "+", "-", "*", "/", "!", "(", ")", ","];

fn tokens(text: &str) -> Result<Vec<(usize, Token)>, String> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let start = at;
        let byte = bytes[at];
        if byte.is_ascii_whitespace() {
            at += 1;
        } else if byte.is_ascii_digit() || (byte == b'.' && bytes.get(at + 1).is_some_and(u8::is_ascii_digit)) {
            while at < bytes.len() && (bytes[at].is_ascii_digit() || bytes[at] == b'.') {
                at += 1;
            }
            let number = text[start..at].parse().map_err(|_| format!("{} is no number (at character {})", &text[start..at], start + 1))?;
            tokens.push((start, Token::Number(number)));
        } else if byte.is_ascii_alphabetic() || byte == b'_' {
            while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
                at += 1;
            }
            tokens.push((start, Token::Name(text[start..at].to_owned())));
        } else if let Some(op) = OPS.iter().find(|op| text[at..].starts_with(**op)) {
            at += op.len();
            tokens.push((start, Token::Op(op)));
        } else {
            let found = text[at..].chars().next().unwrap_or(' ');
            return Err(format!("{found} is no part of a formula (at character {})", start + 1));
        }
    }
    Ok(tokens)
}

struct Parser<'a> {
    tokens: Vec<(usize, Token)>,
    at: usize,
    names: &'a [&'a str],
    end: usize,
}

/// How tightly an operator holds what is beside it, the loosest first.
fn binds(op: &str) -> Option<(u8, Op)> {
    Some(match op {
        "||" => (1, Op::Or),
        "&&" => (2, Op::And),
        "==" => (3, Op::Eq),
        "!=" => (3, Op::NotEq),
        "<" => (4, Op::Less),
        "<=" => (4, Op::LessEq),
        ">" => (4, Op::Greater),
        ">=" => (4, Op::GreaterEq),
        "+" => (5, Op::Add),
        "-" => (5, Op::Sub),
        "*" => (6, Op::Mul),
        "/" => (6, Op::Div),
        _ => return None,
    })
}

impl Parser<'_> {
    fn expression(&mut self, least: u8) -> Result<Expr, String> {
        let mut left = self.operand()?;
        while let Some((_, Token::Op(op))) = self.tokens.get(self.at) {
            let Some((binding, op)) = binds(op).filter(|(binding, _)| *binding >= least) else { break };
            self.at += 1;
            let right = self.expression(binding + 1)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn operand(&mut self) -> Result<Expr, String> {
        let Some((place, token)) = self.tokens.get(self.at).cloned() else {
            return Err(format!("the formula ends where a value is expected (at character {})", self.end + 1));
        };
        self.at += 1;
        match token {
            Token::Number(number) => Ok(Expr::Number(number)),
            Token::Name(name) => match self.names.iter().position(|known| *known == name) {
                Some(at) => Ok(Expr::Name(at)),
                None => Err(format!("no read {name} to compute with: the reads are {}", self.names.join(", "))),
            },
            Token::Op("-") => Ok(Expr::Negative(Box::new(self.expression(7)?))),
            Token::Op("!") => Ok(Expr::Not(Box::new(self.expression(7)?))),
            Token::Op("(") => {
                let inner = self.expression(1)?;
                match self.tokens.get(self.at) {
                    Some((_, Token::Op(")"))) => {
                        self.at += 1;
                        Ok(inner)
                    }
                    _ => Err(format!("a ( opened at character {} is not closed", place + 1)),
                }
            }
            Token::Op(op) => Err(format!("{op} where a value is expected (at character {})", place + 1)),
        }
    }
}

/// `text` as a formula over `names`, each a place in a row; refused with where, and with the names there are
/// when it reaches one that is not among them.
pub fn parse(text: &str, names: &[&str]) -> Result<Expr, String> {
    let mut parser = Parser { tokens: tokens(text)?, at: 0, names, end: text.len() };
    let parsed = parser.expression(1)?;
    match parser.tokens.get(parser.at) {
        None => Ok(parsed),
        Some((place, _)) => Err(format!("the formula goes on past its end (at character {})", place + 1)),
    }
}

/// What `formula` comes to in `row`, none when a value it needs is not there.
pub fn value(formula: &Expr, row: &[Option<f64>]) -> Option<Value> {
    Some(match formula {
        Expr::Number(number) => Value::Number(*number),
        Expr::Name(at) => Value::Number((*row.get(*at)?)?),
        Expr::Negative(inner) => match value(inner, row)? {
            Value::Number(number) => Value::Number(-number),
            Value::Truth(_) => return None,
        },
        Expr::Not(inner) => match value(inner, row)? {
            Value::Truth(truth) => Value::Truth(!truth),
            Value::Number(_) => return None,
        },
        Expr::Binary(op, left, right) => match (value(left, row)?, value(right, row)?) {
            (Value::Number(left), Value::Number(right)) => match op {
                Op::Add => Value::Number(left + right),
                Op::Sub => Value::Number(left - right),
                Op::Mul => Value::Number(left * right),
                Op::Div if right == 0.0 => return None,
                Op::Div => Value::Number(left / right),
                Op::Less => Value::Truth(left < right),
                Op::LessEq => Value::Truth(left <= right),
                Op::Greater => Value::Truth(left > right),
                Op::GreaterEq => Value::Truth(left >= right),
                Op::Eq => Value::Truth(left == right),
                Op::NotEq => Value::Truth(left != right),
                Op::And | Op::Or => return None,
            },
            (Value::Truth(left), Value::Truth(right)) => match op {
                Op::And => Value::Truth(left && right),
                Op::Or => Value::Truth(left || right),
                Op::Eq => Value::Truth(left == right),
                Op::NotEq => Value::Truth(left != right),
                _ => return None,
            },
            _ => return None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_formula_computes_and_a_value_that_is_not_there_stays_absent() {
        let names = ["fails", "flows"];
        let ratio = parse("fails / flows * 100", &names).unwrap();
        assert_eq!(value(&ratio, &[Some(5.0), Some(200.0)]), Some(Value::Number(2.5)));
        assert_eq!(value(&ratio, &[Some(5.0), Some(0.0)]), None, "a division by zero");
        assert_eq!(value(&ratio, &[None, Some(200.0)]), None, "a name with no value");
        let kept = parse("flows >= 20 && !(fails > flows)", &names).unwrap();
        assert_eq!(value(&kept, &[Some(5.0), Some(200.0)]), Some(Value::Truth(true)));
        assert_eq!(value(&parse("-fails + 2 * (flows - 1)", &names).unwrap(), &[Some(1.0), Some(3.0)]), Some(Value::Number(3.0)));
        assert_eq!(value(&parse("1 + 2 * 3 - 4 / 2", &names).unwrap(), &[]), Some(Value::Number(5.0)));
    }

    #[test]
    fn a_mistake_is_refused_with_where_before_any_row() {
        let names = ["fails", "flows"];
        let refused = |text: &str| parse(text, &names).unwrap_err();
        assert_eq!(refused("fails / flow"), "no read flow to compute with: the reads are fails, flows");
        assert_eq!(refused("fails /"), "the formula ends where a value is expected (at character 8)");
        assert_eq!(refused("(fails + 1"), "a ( opened at character 1 is not closed");
        assert_eq!(refused("fails ; flows"), "; is no part of a formula (at character 7)");
        assert_eq!(refused("fails flows"), "the formula goes on past its end (at character 7)");
    }
}

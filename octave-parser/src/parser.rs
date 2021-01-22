use crate::{
    ast::{Expr, Op, Statement},
    node::{Node, Position},
    value::Matrix,
};
use pest::{
    iterators::{Pair, Pairs},
    prec_climber::{Assoc, Operator, PrecClimber},
    Parser, Span,
};
use std::{collections::HashSet, ops::Range};

#[derive(Copy, Clone, Debug, Parser)]
#[grammar = "grammar.pest"]
pub struct OctaveParser;

pub fn parse(input: &str) -> Node<Statement> {
    OctaveParser::parse(Rule::toplevel, input)
        .map(|pairs: Pairs<Rule>| process_stmt(pairs.into_iter().next().unwrap()))
        .unwrap_or_else(|e: pest::error::Error<Rule>| Node {
            span: Position { line: 1, col: 1 }..Position {
                line: 1,
                col: input.lines().next().unwrap().len(),
            },
            data: Statement::Error(format!(
                "Parse error: {}",
                match e.variant {
                    pest::error::ErrorVariant::CustomError { message } => message,
                    pest::error::ErrorVariant::ParsingError {
                        negatives,
                        positives,
                    } => format!("Unexpected {:?}, expected {:?}", negatives, positives),
                }
            )),
        })
}

fn process_stmt(pair: Pair<Rule>) -> Node<Statement> {
    match pair.as_rule() {
        Rule::toplevel => Node {
            span: to_range(pair.as_span()),
            data: Statement::Block(pair.into_inner().map(process_stmt).collect()),
        },
        Rule::assignment => Node {
            span: to_range(pair.as_span()),
            data: {
                let mut it = pair.into_inner();
                let ident = it.next().unwrap().as_str().into();
                let expr = it.next().map(process_expr).unwrap();
                Statement::Assignment(ident, expr)
            },
        },
        Rule::statement_semi => Node {
            span: to_range(pair.as_span()),
            data: Statement::IgnoreOutput(
                pair.into_inner()
                    .map(process_stmt)
                    .map(|n| n.map(Box::new))
                    .next()
                    .unwrap(),
            ),
        },
        Rule::expr => Node {
            span: to_range(pair.as_span()),
            data: Statement::Expr(process_expr(pair.into_inner().next().unwrap()))
        },
        Rule::EOI => Node {
            span: to_range(pair.as_span()),
            data: Statement::EOI,
        },
        r => Node {
            span: to_range(pair.as_span()),
            data: Statement::Error(format!("Parse error, unexpected {:?}", r)),
        },
    }
}

fn process_expr(pair: Pair<Rule>) -> Node<Expr> {
    lazy_static! {
        static ref PREC: PrecClimber<Rule> = PrecClimber::new(vec![
            Operator::new(Rule::add, Assoc::Left) | Operator::new(Rule::sub, Assoc::Left),
            Operator::new(Rule::mul, Assoc::Left) | Operator::new(Rule::div, Assoc::Left),
            Operator::new(Rule::pow, Assoc::Right),
            Operator::new(Rule::access, Assoc::Right),
        ]);
    }
    match pair.as_rule() {
        Rule::expr => process_expr(pair.into_inner().next().unwrap()),
        Rule::single_value => Node {
            span: to_range(pair.as_span()),
            data: pair
                .into_inner()
                .map(process_expr)
                .map(|v| Expr::Matrix(Matrix::from_vecs(vec![vec![v]])))
                .next()
                .unwrap_or(Expr::Error("Syntax error".into())),
        },
        Rule::binary => PREC.climb(
            pair.into_inner(),
            process_expr,
            |lhs: Node<Expr>, op: Pair<Rule>, rhs: Node<Expr>| Node {
                span: union(lhs.span.clone(), rhs.span.clone()),
                data: match get_op(op.as_rule()) {
                    Ok(op) => Expr::Op(op, lhs.map(Box::new), rhs.map(Box::new)),
                    Err(rule) => Expr::Error(format!("Unexpected {:?}", rule)),
                },
            },
        ),
        Rule::call => Node {
            span: to_range(pair.as_span()),
            data: {
                let mut it = pair.into_inner().map(process_expr);
                let ident = it.next().unwrap();
                let exprs = it.collect();
                Expr::Call(ident.map(Box::new), exprs)
            },
        },
        Rule::matrix => Node {
            span: to_range(pair.as_span()),
            data: {
                let data: Vec<Vec<_>> = pair
                    .into_inner()
                    .map(|line| line.into_inner().map(process_expr).collect())
                    .collect();
                let innerlen = data[0].len();
                if !data.iter().all(|v| v.len() == innerlen) {
                    let sizes = data.iter().map(|v| v.len()).collect::<HashSet<_>>();
                    Expr::Error(format!(
                        "Matrix sizing error: found lines of sizes {:?}",
                        sizes
                    ))
                } else {
                    Expr::Matrix(Matrix::from_vecs(data))
                }
            },
        },
        Rule::identifier => Node {
            span: to_range(pair.as_span()),
            data: Expr::Identifier(pair.as_str().to_string()),
        },
        Rule::string => Node {
            span: to_range(pair.as_span()),
            data: Expr::LitString(
                pair.into_inner()
                    .map(|x| match x.as_str() {
                        "\\\"" => "\"".into(),
                        x => x.to_string(),
                    })
                    .flat_map(|x| x.chars().collect::<Vec<_>>().into_iter())
                    .collect(),
            ),
        },
        Rule::number => Node {
            span: to_range(pair.as_span()),
            data: pair
                .as_str()
                .parse()
                .map(Expr::LitNumber)
                .unwrap_or(Expr::Error("Cannot parse number".into())),
        },
        _ => Node {
            span: to_range(pair.as_span()),
            data: Expr::Error(format!("Syntax error, unexpected {:?}", pair.as_rule())),
        },
    }
}

fn union<T: Ord>(p0: Range<T>, p1: Range<T>) -> Range<T> {
    use std::cmp::{max, min};
    min(p0.start, p1.start)..max(p0.end, p1.end)
}

fn get_op(rule: Rule) -> Result<Op, Rule> {
    Ok(match rule {
        Rule::add => Op::Add,
        Rule::sub => Op::Sub,
        Rule::mul => Op::Mul,
        Rule::div => Op::Div,
        Rule::pow => Op::Pow,
        Rule::access => Op::Access,
        _ => return Err(rule),
    })
}

fn to_range(span: Span) -> Range<Position> {
    to_pos(span.start_pos())..to_pos(span.end_pos())
}

fn to_pos(p: pest::Position) -> Position {
    let (line, col) = p.line_col();
    Position { line, col }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::{
        ast::{Expr, Statement},
        node::{Node, Position},
        value::Matrix,
    };
    use std::ops::Deref;

    #[test]
    fn number() {
        let actual = parse("42.0");
        let expected_mat = Some(Matrix::from_vecs(vec![vec![42.0]]));
        let actual_mat = actual.get_matrix();
        println!("{:#?}", actual);
        assert_eq!(actual.as_ref().get_errors().len(), 0);
        assert_eq!(expected_mat, actual_mat);
    }

    #[test]
    fn matrix1() {
        let actual = parse("[1 2 3; 4 5 6]");
        let expected_mat = Some(Matrix::from_vecs(vec![
            vec![1.0, 2.0, 3.0],
            vec![4.0, 5.0, 6.0],
        ]));
        let actual_mat = actual.get_matrix();
        println!("{:#?}", actual);
        assert_eq!(expected_mat, actual_mat);
    }

    #[test]
    fn matrix2() {
        let actual = parse(r#"["hello" "internet"]"#);
        let expected_mat = Some(Matrix::from_vecs(vec![vec!["hello", "internet"]]));
        let actual_mat = actual.get_str_matrix();
        println!("{:#?}", actual);
        assert_eq!(expected_mat, actual_mat);
    }

    #[test]
    fn matrix3() {
        let actual = parse("[a b c]");
        let expected_mat = Matrix::from_vecs(vec![vec!["a", "b", "c"]]);
        println!("{:#?}", actual);
        let actual_mat = if let Statement::Block(v) = actual.deref() {
            if let Statement::Expr(Expr::Matrix(m)) = v[0].deref() {
                m.as_ref().map(|n| {
                    if let Expr::Identifier(a) = n.deref() {
                        a.as_str()
                    } else {
                        unreachable!()
                    }
                })
            } else { unreachable!() }
        } else {
            unreachable!()
        };

        assert_eq!(expected_mat, actual_mat);
    }

    #[test]
    fn matrix_mismatch() {
        let actual = parse("[1 2 3; 4 5]");
        println!("{:#?}", actual);
        assert_eq!(1, actual.as_ref().get_errors().len());
    }

    #[test]
    fn assignment() {
        let actual = parse("hello = [1 2 3]");
        println!("{:#?}", actual);
        assert_eq!(0, actual.as_ref().get_errors().len());
    }

    #[test]
    fn operation() {
        let actual = parse("[1 2 3] + [4 5 6]");
        println!("{:#?}", actual);
        assert_eq!(0, actual.as_ref().get_errors().len());
    }

    #[test]
    fn call() {
        let actual = parse("sin([1 2 3 4 5 6])");
        println!("{:#?}", actual);
        assert_eq!(0, actual.as_ref().get_errors().len());
    }

    #[test]
    fn stmt_block() {
        let actual = parse("[1 2 3]\n[4 5 6];");
        let actual_vals = if let Statement::Block(v) = actual.deref() {
            v.iter()
                .take_while(|n| {
                    if let Statement::EOI = Node::deref(*n) {
                        false
                    } else {
                        true
                    }
                })
                .map(|v| v.get_matrix().unwrap())
                .collect()
        } else {
            vec![]
        };
        println!("{:#?}", actual);
        println!("Errors: {:?}", actual.as_ref().get_errors());
        assert_eq!(0, actual.as_ref().get_errors().len());
    }
}

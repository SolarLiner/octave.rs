use crate::node::Node;
use crate::value::Matrix;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Access,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Error(String),
    LitString(String),
    LitNumber(f64),
    Identifier(String),
    Matrix(Matrix<Node<Expr>>),
    Op(Op, Node<Box<Expr>>, Node<Box<Expr>>),
    Incr(Node<Box<Expr>>),
    Decr(Node<Box<Expr>>),
    Range(Node<Box<Expr>>, Option<Node<Box<Expr>>>, Node<Box<Expr>>),
    Call(Node<Box<Expr>>, Vec<Node<Expr>>),
}

impl Expr {
    pub(crate) fn get_str_matrix(&self) -> Option<Matrix<&str>> {
        match self {
            Expr::LitString(s) => Some(Matrix::from_vecs(vec![vec![s.as_str()]])),
            Expr::Matrix(m) => m.as_ref().map(|Node {data, ..}| data.get_str()).transpose(),
            _ => None,
        }
    }
    fn get_str(&self) -> Option<&str> {
        match self {
            Expr::LitString(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

impl Expr {
    pub fn get_value(&self) -> Option<f64> {
        match self {
            Expr::LitNumber(v) => Some(*v),
            _ => None,
        }
    }
    pub(crate) fn get_matrix(&self) -> Option<Matrix<f64>> {
        match self {
            Expr::Matrix(m) => m.as_ref().map(|e| e.data.get_value()).transpose(),
            _ => None,
        }
    }
}

impl<'a> Node<&'a Expr> {
    pub fn get_errors(&self) -> Vec<Node<String>> {
        match &self.data {
            Expr::Error(s) => vec![Node {
                span: self.span.clone(),
                data: s.clone(),
            }],
            Expr::Matrix(m) => m
                .data
                .iter()
                .flat_map(|n| n.as_ref().get_errors().into_iter())
                .collect(),
            Expr::Decr(n) => n.as_deref().get_errors(),
            Expr::Incr(n) => n.as_deref().get_errors(),
            Expr::Range(start, range, end) => start
                .as_deref()
                .get_errors()
                .into_iter()
                .chain(
                    range
                        .as_ref()
                        .map(|n| n.as_deref().get_errors())
                        .unwrap_or(vec![])
                        .into_iter(),
                )
                .chain(end.as_deref().get_errors().into_iter())
                .collect(),
            _ => vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Statement {
    Error(String),
    IgnoreOutput(Node<Box<Statement>>),
    Expr(Expr),
    Assignment(String, Node<Expr>),
    AugAssignment(String, Op, Node<Expr>),
    Block(Vec<Node<Statement>>),
    EOI,
}

impl Statement {
    pub fn get_matrix(&self) -> Option<Matrix<f64>> {
        match self {
            Statement::Expr(e) => e.get_matrix(),
            Statement::Block(v) => v[0].get_matrix(),
            Statement::IgnoreOutput(s) => s.get_matrix(),
            _ => None,
        }
    }

    pub fn get_str_matrix(&self) -> Option<Matrix<&str>> {
        match self {
            Statement::Expr(e) => e.get_str_matrix(),
            Statement::Block(v) => v[0].get_str_matrix(),
            Statement::IgnoreOutput(s) => s.get_str_matrix(),
            _ => None,
        }
    }
}

impl<'a> Node<&'a Statement> {
    pub fn get_errors(&'a self) -> Vec<Node<String>> {
        match &self.data {
            Statement::Error(s) => vec![Node {
                span: self.span.clone(),
                data: s.clone(),
            }],
            Statement::IgnoreOutput(s) => s.as_deref().get_errors(),
            Statement::Expr(e) => (Node {
                span: self.span.clone(),
                data: e,
            })
            .get_errors(),
            Statement::Assignment(_, e) => e.as_ref().get_errors(),
            Statement::AugAssignment(_, _, e) => e.as_ref().get_errors(),
            Statement::Block(vs) => vs
                .iter()
                .flat_map(|n| n.as_ref().get_errors().into_iter())
                .collect(),
            Statement::EOI => vec![],
        }
    }
}

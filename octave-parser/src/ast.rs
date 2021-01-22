use crate::node::{Node, Tree};
use crate::value::Matrix;
use octave_typesystem::{SimpleType, Type};
use flurry::{HashMapRef};
use thiserror::Error;
use std::ops::Deref;

#[derive(Clone, Debug, Error)]
pub enum TypeError {
    #[error("Type mismatch between {0} and {1}")]
    TypeMismatch(Type, Type),
    #[error("Nested value type {0}")]
    NestedType(Type),
    #[error("Type {0} is not callable")]
    NotCallable(Type),
}

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

impl Tree for Expr {
    type Item = Node<Self>;

    fn children(&self) -> Vec<Self::Item> {
        match self {
            Self::Matrix(m) => m.data.clone(),
            Self::Op(_, a, b) => vec![
                a.as_deref().map(|v| v.clone()),
                b.as_deref().map(|v| v.clone()),
            ],
            Self::Range(s, Some(st), e) => vec![
                s.as_deref().map(Clone::clone),
                st.as_deref().map(Clone::clone),
                e.as_deref().map(Clone::clone),
            ],
            Self::Range(s, None, e) => vec![
                s.as_deref().map(Clone::clone),
                e.as_deref().map(Clone::clone),
            ],
            Self::Call(c, v) => std::iter::once(c.as_deref().map(Clone::clone))
                .chain(v.iter().map(|n| n.as_ref().map(Clone::clone)))
                .collect(),
            Self::Decr(e) | Self::Incr(e) => vec![e.as_deref().map(Clone::clone)],
            _ => vec![],
        }
    }
}

impl Expr {
    pub(crate) fn get_str_matrix(&self) -> Option<Matrix<&str>> {
        match self {
            Expr::LitString(s) => Some(Matrix::from_vecs(vec![vec![s.as_str()]])),
            Expr::Matrix(m) => m
                .as_ref()
                .map(|Node { data, .. }| data.get_str())
                .transpose(),
            _ => None,
        }
    }

    fn get_str(&self) -> Option<&str> {
        match self {
            Expr::LitString(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn type_of(&self, ctx: HashMapRef<String, Type>) -> Type {
        match self {
            Self::LitString(_) => Type::SimpleType(SimpleType::String),
            Self::LitNumber(_) => Type::SimpleType(SimpleType::Double),
            Self::Range(s, _, _) => Type::Matrix {
                size: None,
                ty: s.type_of(ctx).simple_type().unwrap_or(SimpleType::Unknown),
            },
            Self::Incr(_) | Self::Decr(_) => Type::SimpleType(SimpleType::Void),
            Self::Call(c, _) => {
                if let Type::Callable(c) = c.type_of(ctx) {
                    (*c.return_type).clone()
                } else {
                    Type::Unknown
                }
            }
            Self::Op(_, a, _) => a.type_of(ctx),
            Self::Identifier(i) => ctx.get(i).cloned().unwrap_or(Type::Unknown),
            Self::Error(_) => Type::Unknown,
            Self::Matrix(m) => Type::Matrix {
                size: Some((m.width(), m.height())),
                ty: m.data[0]
                    .type_of(ctx)
                    .simple_type()
                    .unwrap_or(SimpleType::Unknown),
            },
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
    Expr(Node<Expr>),
    Assignment(String, Node<Expr>),
    AugAssignment(String, Op, Node<Expr>),
    Block(Vec<Node<Statement>>),
    EOI,
}

impl Tree for Statement {
    type Item = Node<Self>;

    fn children(&self) -> Vec<Self::Item> {
        match self {
            Self::Block(v) => v.iter().map(|v| v.clone()).collect(),
            Self::IgnoreOutput(n) => vec![n.as_deref().map(Clone::clone)],
            _ => vec![],
        }
    }
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

    pub fn add_bindings(&self, ctx: HashMapRef<String, Type>) {
        match self {
            Self::Assignment(i, e) => {
                ctx.insert(i.clone(), e.type_of(ctx.clone()));
            },
            Self::Block(v) => {
                for s in v {
                    s.add_bindings(ctx.clone());
                }
            },
            Self::IgnoreOutput(s) => s.add_bindings(ctx),
            _ => {}
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
            Statement::Expr(e) => e.as_ref().get_errors(),
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

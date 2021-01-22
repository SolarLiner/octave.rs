use crate::ast::{Expr, Statement};
use lsp_types as lsp;
use std::ops::{Deref, Range};

pub trait Tree: Clone {
    type Item;
    fn children(&self) -> Vec<Self::Item>;
}

impl<'a, T: Tree> Tree for &'a T {
    type Item = T::Item;

    fn children(&self) -> Vec<Self::Item> {
        T::children(self)
    }
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct Position {
    pub line: usize,
    pub col: usize,
}

impl From<lsp::Position> for Position {
    fn from(s: lsp::Position) -> Self {
        Self {
            line: (s.line + 1) as usize,
            col: (s.character + 1) as usize,
        }
    }
}

impl From<Position> for lsp::Position {
    fn from(p: Position) -> Self {
        Self {
            line: (p.line - 1) as u64,
            character: (p.col - 1) as u64,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node<T> {
    pub(crate) span: Range<Position>,
    pub(crate) data: T,
}

impl<T> Node<T> {
    pub fn span(&self) -> Range<Position> {
        self.span.clone()
    }
}

impl<T> Deref for Node<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> Node<T> {
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Node<U> {
        Node {
            span: self.span,
            data: f(self.data),
        }
    }

    pub fn as_ref(&self) -> Node<&T> {
        Node {
            span: self.span.clone(),
            data: &self.data,
        }
    }
}

impl<T, B: Deref<Target = T>> Node<B> {
    pub fn as_deref(&self) -> Node<&T> {
        Node {
            span: self.span.clone(),
            data: self.data.deref(),
        }
    }
}

impl<T, E> Node<Result<T, E>> {
    pub fn map_data<U, F: FnOnce(T) -> U>(self, f: F) -> Node<Result<U, E>> {
        self.map(|data| data.map(f))
    }
    pub fn and_then<U, F: FnOnce(T) -> Result<U, E>>(self, f: F) -> Node<Result<U, E>> {
        self.map(|data| data.and_then(f))
    }
    pub fn transpose(self) -> Result<Node<T>, Node<E>> {
        match self.data {
            Ok(data) => Ok(Node {
                span: self.span,
                data,
            }),
            Err(data) => Err(Node {
                span: self.span,
                data,
            }),
        }
    }
}

impl<T> Node<Option<T>> {
    pub fn map_data<U, F: FnOnce(T) -> U>(self, f: F) -> Node<Result<U, String>> {
        self.map(|data| match data {
            None => Err("Unexpected end of file".to_string()),
            Some(x) => Ok(f(x)),
        })
    }
    pub fn and_then<U, F: FnOnce(T) -> Option<U>>(self, f: F) -> Node<Option<U>> {
        self.map(|data| data.and_then(f))
    }

    pub fn transpose(self) -> Option<Node<T>> {
        match self.data {
            Some(data) => Some(Node {
                span: self.span,
                data,
            }),
            None => None,
        }
    }
}

impl Node<&Expr> {
    pub fn at_pos(&self, pos: Position) -> Option<Node<Expr>> {
        if self.span.contains(&pos) {
            Some(match self.data {
                Expr::Error(_) | Expr::Identifier(_) | Expr::LitNumber(_) | Expr::LitString(_) => Some(self.clone().map(Clone::clone)),
                Expr::Matrix(m) => m.iter().filter_map(|n| n.as_ref().at_pos(pos)).next(),
                Expr::Op(_, a, b) => a.as_deref().at_pos(pos).or_else(|| b.as_deref().at_pos(pos)),
                Expr::Call(c, v) => c.as_deref().at_pos(pos).or_else(|| v.iter().filter_map(|n| n.as_ref().at_pos(pos)).next()),
                Expr::Decr(e) | Expr::Incr(e) => e.as_deref().at_pos(pos),
                Expr::Range(s, st, e) => s.as_deref().at_pos(pos).or_else(|| st.as_ref().and_then(|n| n.as_deref().at_pos(pos))).or_else(|| e.as_deref().at_pos(pos)),
            }.unwrap_or(self.clone().map(Clone::clone)))
        } else {
            None
        }
    }
}

impl Node<&Statement> {
    pub fn at_pos(&self, pos: Position) -> Option<Node<Expr>> {
        if self.span.contains(&pos) {
            match self.data {
                Statement::Expr(e) => e.as_ref().at_pos(pos),
                Statement::Assignment(_, e) => e.as_ref().at_pos(pos),
                Statement::AugAssignment(_, _, e) => e.as_ref().at_pos(pos),
                Statement::Block(v) => v.iter().filter_map(|n| n.as_ref().at_pos(pos)).next(),
                Statement::IgnoreOutput(e) => e.as_deref().at_pos(pos),
                Statement::EOI | Statement::Error(_) => None,
            }
        } else {
            None
        }
    }
}

impl Node<Statement> {
    pub fn at_pos(&self, pos: Position) -> Option<Node<Expr>> {
        self.as_ref().at_pos(pos)
    }
}

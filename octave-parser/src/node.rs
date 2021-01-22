use std::ops::{Deref, Range};

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct Position {
    pub line: usize,
    pub col: usize,
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

impl<T, B: Deref<Target=T>> Node<B> {
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

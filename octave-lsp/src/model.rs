use std::ops::{Deref, Range};

use flurry::HashMap;
use tower_lsp::lsp_types as lsp;
use tower_lsp::lsp_types::{Diagnostic, TextDocumentContentChangeEvent, TextEdit, Url};

use flurry::epoch::Guard;
use lsp_textdocument::{TextDocument, TextDocumentMutationError};
use octave_parser::ast::{Expr, Statement};
use octave_parser::node::{Node, Position};
use octave_parser::parser::parse;
use std::borrow::BorrowMut;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum ModelError {
    #[error("Text document error: {0}")]
    TextDocumentError(#[from] TextDocumentMutationError),
    #[error("Unknown document: {0}")]
    UnknownDocument(Url),
}

#[derive(Debug, Default)]
pub struct Model {
    documents: HashMap<Url, (TextDocument, Node<Statement>)>,
}

impl Model {
    #[allow(dead_code)]
    pub fn document<'g>(
        &'g self,
        uri: &Url,
        guard: &'g Guard,
    ) -> Option<(&'g str, Node<&'g Statement>)> {
        self.documents
            .get(uri, guard)
            .map(|(s, n)| (s.text(), n.as_ref()))
    }

    pub fn guard(&self) -> Guard {
        self.documents.guard()
    }

    pub fn apply_edits(
        &self,
        uri: &Url,
        changes: Vec<TextDocumentContentChangeEvent>,
        version: Option<i64>,
    ) -> Result<(), ModelError> {
        self.documents
            .pin()
            .compute_if_present(uri, |_, (doc, ast)| {
                let mut doc = doc.clone();
                doc.update(changes, version);
                let ast = parse(doc.deref());
                Some((doc, ast))
            })
            .map(|_| ())
            .ok_or(ModelError::UnknownDocument(uri.clone()))
    }

    pub fn set_document(&self, uri: Url, text: String) {
        let ast = parse(text.as_str());
        let guard = self.documents.guard();
        let document = TextDocument::new(uri.clone(), "octave", 0, text);
        self.documents.insert(uri, (document, ast), &guard);
    }

    pub fn get_variables(&self) -> Vec<String> {
        let guard = self.documents.guard();
        self.documents
            .values(&guard)
            .flat_map(|(_, s)| Self::get_variables_inner(s.deref()).into_iter())
            .collect()
    }

    pub fn get_diagnostics(&self, uri: &Url) -> Vec<Diagnostic> {
        let guard = self.documents.guard();
        if let Some((_, ast)) = self.documents.get(uri, &guard) {
            Self::get_diagnostics_stmt(ast.deref(), ast.span())
        } else {
            vec![]
        }
    }

    fn get_variables_inner(stmt: &Statement) -> Vec<String> {
        match stmt {
            Statement::Assignment(s, _) => vec![s.clone()],
            Statement::IgnoreOutput(s) => Self::get_variables_inner(s.as_deref().deref()),
            Statement::Block(v) => v
                .iter()
                .flat_map(|n| Self::get_variables_inner(n.deref()).into_iter())
                .collect(),
            _ => vec![],
        }
    }

    fn get_diagnostics_stmt(stmt: &Statement, span: Range<Position>) -> Vec<Diagnostic> {
        match stmt {
            Statement::Error(s) => vec![Diagnostic::new(
                parser_range_to_lsp_range(span),
                lsp::DiagnosticSeverity::Error.into(),
                None,
                Some("Octave".into()),
                s.clone(),
                None,
                None,
            )],
            Statement::Block(v) => v
                .iter()
                .flat_map(|n| Self::get_diagnostics_stmt(n.deref(), n.span()))
                .collect(),
            Statement::IgnoreOutput(s) => Self::get_diagnostics_stmt(s.deref(), s.span()),
            Statement::Assignment(_, e) | Statement::AugAssignment(_, _, e) => {
                Self::get_diagnostics_expr(e.deref(), e.span())
            }
            Statement::Expr(e) => Self::get_diagnostics_expr(e, span),
            Statement::EOI => vec![],
        }
    }

    fn get_diagnostics_expr(expr: &Expr, span: Range<Position>) -> Vec<Diagnostic> {
        match expr {
            Expr::Range(s, st, e) => Self::get_diagnostics_expr(s.deref(), s.span())
                .into_iter()
                .chain(
                    st.as_ref()
                        .map(|n| Self::get_diagnostics_expr(n.deref(), n.span()))
                        .unwrap_or(vec![])
                        .into_iter(),
                )
                .chain(Self::get_diagnostics_expr(e.deref(), e.span()).into_iter())
                .collect(),
            Expr::Error(s) => vec![lsp::Diagnostic::new(
                parser_range_to_lsp_range(span),
                lsp::DiagnosticSeverity::Error.into(),
                None,
                Some("Octave".into()),
                s.clone(),
                None,
                None,
            )],
            Expr::Op(_, a, b) => Self::get_diagnostics_expr(a.deref(), a.span())
                .into_iter()
                .chain(Self::get_diagnostics_expr(b.deref(), b.span()).into_iter())
                .collect(),
            Expr::Matrix(m) => m
                .as_ref()
                .map(|n| Self::get_diagnostics_expr(n.deref(), n.span()))
                .into_iter()
                .flat_map(|v| v.into_iter())
                .collect(),
            Expr::Call(s, e) => Self::get_diagnostics_expr(s.as_deref().deref(), s.span())
                .into_iter()
                .chain(
                    e.iter()
                        .flat_map(|v| Self::get_diagnostics_expr(v.deref(), v.span()).into_iter()),
                )
                .collect(),
            Expr::Decr(e) | Expr::Incr(e) => {
                Self::get_diagnostics_expr(e.as_deref().deref(), e.span())
            }
            _ => vec![],
        }
    }
}

fn parser_range_to_lsp_range(range: Range<Position>) -> lsp::Range {
    lsp::Range {
        start: parser_pos_to_lsp_pos(range.start),
        end: parser_pos_to_lsp_pos(range.end),
    }
}

fn parser_pos_to_lsp_pos(pos: Position) -> lsp::Position {
    lsp::Position {
        line: pos.line as u64 - 1,
        character: pos.col as u64 - 1,
    }
}

use std::ops::{Deref, Range};

use flurry::HashMap;
use tower_lsp::lsp_types as lsp;
use tower_lsp::lsp_types::{Diagnostic, TextDocumentContentChangeEvent, TextEdit, Url};

use flurry::epoch::Guard;
use lsp_textdocument::{TextDocument, TextDocumentMutationError};
use octave_parser::ast::{Expr, Statement};
use octave_parser::node::{Node, Position};
use octave_parser::parser::parse;
use octave_typesystem::{CallableType, SimpleType, Type};
use std::borrow::BorrowMut;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum ModelError {
    #[error("Text document error: {0}")]
    TextDocumentError(#[from] TextDocumentMutationError),
    #[error("Unknown document: {0}")]
    UnknownDocument(Url),
}

#[derive(Debug)]
pub struct DocumentData {
    pub doc: TextDocument,
    pub ast: Node<Statement>,
    pub bindings: HashMap<String, Type>,
}

#[derive(Debug, Default)]
pub struct Model {
    documents: HashMap<Url, DocumentData>,
}

impl Model {
    #[allow(dead_code)]
    pub fn document<'g>(&'g self, uri: &Url, guard: &'g Guard) -> Option<&'g DocumentData> {
        self.documents.get(uri, guard)
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
            .compute_if_present(uri, |_, DocumentData { doc, .. }| {
                let mut doc = doc.clone();
                doc.update(changes, version);
                let ast = parse(doc.deref());
                let bindings = get_bindings(ast.as_ref());
                Some(DocumentData { doc, ast, bindings })
            })
            .map(|_| ())
            .ok_or(ModelError::UnknownDocument(uri.clone()))
    }

    pub fn set_document(&self, uri: Url, text: String) {
        let ast = parse(text.as_str());
        let bindings = get_bindings(ast.as_ref());
        let guard = self.documents.guard();
        let doc = TextDocument::new(uri.clone(), "octave", 0, text);
        self.documents
            .insert(uri, DocumentData { doc, ast, bindings }, &guard);
    }

    pub fn get_variables(&self) -> Vec<(String, Type)> {
        self.documents
            .pin()
            .values()
            .flat_map(|data| {
                data.bindings
                    .pin()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<Vec<_>>()
                    .into_iter()
            })
            .collect()
    }

    pub fn get_diagnostics(&self, uri: &Url) -> Vec<Diagnostic> {
        let guard = self.documents.guard();
        if let Some(data) = self.documents.get(uri, &guard) {
            Self::get_diagnostics_stmt(data.ast.as_ref())
        } else {
            vec![]
        }
    }

    fn get_diagnostics_stmt(node: Node<&Statement>) -> Vec<Diagnostic> {
        match node.deref() {
            Statement::Error(s) => vec![Diagnostic::new(
                parser_range_to_lsp_range(node.span()),
                lsp::DiagnosticSeverity::Error.into(),
                None,
                Some("Octave".into()),
                s.clone(),
                None,
                None,
            )],
            Statement::Block(v) => v
                .iter()
                .flat_map(|n| Self::get_diagnostics_stmt(n.as_ref()))
                .collect(),
            Statement::IgnoreOutput(s) => Self::get_diagnostics_stmt(s.as_deref()),
            Statement::Assignment(_, e) | Statement::AugAssignment(_, _, e) => {
                Self::get_diagnostics_expr(e.as_ref())
            }
            Statement::Expr(e) => Self::get_diagnostics_expr(e.as_ref()),
            Statement::EOI => vec![],
        }
    }

    fn get_diagnostics_expr(node: Node<&Expr>) -> Vec<Diagnostic> {
        match node.deref() {
            Expr::Range(s, st, e) => Self::get_diagnostics_expr(s.as_deref())
                .into_iter()
                .chain(
                    st.as_ref()
                        .map(|n| Self::get_diagnostics_expr(n.as_deref()))
                        .unwrap_or(vec![])
                        .into_iter(),
                )
                .chain(Self::get_diagnostics_expr(e.as_deref()).into_iter())
                .collect(),
            Expr::Error(s) => vec![lsp::Diagnostic::new(
                parser_range_to_lsp_range(node.span()),
                lsp::DiagnosticSeverity::Error.into(),
                None,
                Some("Octave".into()),
                s.clone(),
                None,
                None,
            )],
            Expr::Op(_, a, b) => Self::get_diagnostics_expr(a.as_deref())
                .into_iter()
                .chain(Self::get_diagnostics_expr(b.as_deref()).into_iter())
                .collect(),
            Expr::Matrix(m) => m
                .as_ref()
                .map(|n| Self::get_diagnostics_expr(n.as_ref()))
                .into_iter()
                .flat_map(|v| v.into_iter())
                .collect(),
            Expr::Call(s, e) => Self::get_diagnostics_expr(s.as_deref())
                .into_iter()
                .chain(
                    e.iter()
                        .flat_map(|v| Self::get_diagnostics_expr(v.as_ref()).into_iter()),
                )
                .collect(),
            Expr::Decr(e) | Expr::Incr(e) => Self::get_diagnostics_expr(e.as_deref()),
            _ => vec![],
        }
    }
}

fn get_prelude() -> HashMap<String, Type> {
    let map = HashMap::new();
    {
        let map = map.pin();
        let trig_fn_type = Type::Callable(CallableType {
            args_types: vec![Type::Matrix {
                size: None,
                ty: SimpleType::Double,
            }],
            return_type: Box::new(Type::Matrix {
                size: None,
                ty: SimpleType::Double,
            }),
        });
        map.insert("sin".into(), trig_fn_type.clone());
        map.insert("cos".into(), trig_fn_type.clone());
        map.insert("tan".into(), trig_fn_type.clone());
        map.insert(
            "sound".into(),
            Type::Callable(CallableType {
                args_types: vec![
                    Type::Matrix {
                        size: None,
                        ty: SimpleType::Double,
                    },
                    Type::Matrix {
                        size: Some((1, 1)),
                        ty: SimpleType::Double,
                    },
                ],
                return_type: Box::new(Type::SimpleType(SimpleType::Void)),
            }),
        );
    }

    map
}

fn get_bindings(ast: Node<&Statement>) -> HashMap<String, Type> {
    let bindings = get_prelude();
    ast.add_bindings(bindings.pin());
    bindings
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

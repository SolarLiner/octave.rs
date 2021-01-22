use std::fmt;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SimpleType {
    Void,
    Single,
    Double,
    String,
    Unknown,
}

impl SimpleType {
    pub fn is_scalar(&self) -> bool {
        match self {
            Self::Single | Self::Double => true,
            _ => false,
        }
    }
}

impl fmt::Display for SimpleType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Void => write!(f, "void"),
            Self::Single => write!(f, "single"),
            Self::Double => write!(f, "double"),
            Self::String => write!(f, "string"),
            Self::Unknown => write!(f, "?"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableType {
    pub args_types: Vec<Type>,
    pub return_type: Box<Type>,
}

impl CallableType {
    pub fn is_scalar(&self) -> bool {
        self.return_type.is_scalar()
    }
}

impl fmt::Display for CallableType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(")?;
        let mut first = true;
        for c in &self.args_types {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "{}", c)?;
            first = false;
        }
        write!(f, ") -> {}", self.return_type)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    SimpleType(SimpleType),
    Matrix {
        size: Option<(usize, usize)>,
        ty: SimpleType,
    },
    Callable(CallableType),
    Unknown,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::SimpleType(s) => s.fmt(f),
            Self::Matrix { size: Some((rows, cols)), ty } => write!(f, "{}x{} {} matrix", rows, cols, ty),
            Self::Matrix { ty , ..} => write!(f, "{} matrix", ty),
            Self::Callable(c) => c.fmt(f),
            Self::Unknown => write!(f, "?"),
        }
    }
}

impl Type {
    pub fn is_scalar(&self) -> bool {
        match self {
            Self::SimpleType(s) => s.is_scalar(),
            Self::Callable(c) => c.is_scalar(),
            _ => false,
        }
    }

    pub fn simple_type(&self) -> Option<SimpleType> {
        match self {
            Self::SimpleType(s) => Some(*s),
            Self::Callable(c) if c.is_scalar() => c.return_type.simple_type(),
            _ => None,
        }
    }
}

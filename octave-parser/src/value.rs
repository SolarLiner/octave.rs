use std::ops::{Index, IndexMut, Deref};

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct Matrix<T> {
    pub(crate) data: Vec<T>,
    width: usize,
}

impl<T> Matrix<T> {
    pub fn from_vecs(data: Vec<Vec<T>>) -> Self {
        let len = data[0].len();
        Self {
            data: data.into_iter().flat_map(|v| v.into_iter()).collect(),
            width: len,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.data.len() / self.width
    }

    pub fn ix(&self, i: usize, j: usize) -> usize {
        self.width * j + i
    }

    pub fn get(&self, i: usize, j: usize) -> Option<&T> {
        self.data.get(self.ix(i, j))
    }

    pub fn get_mut(&mut self, i: usize, j: usize) -> Option<&mut T> {
        let idx = self.ix(i, j);
        self.data.get_mut(idx)
    }

    pub fn set(&mut self, i: usize, j: usize, v: T) -> bool {
        let idx = self.ix(i, j);
        if idx < self.data.len() {
            self.data[idx] = v;
            true
        } else {
            false
        }
    }

    pub fn map<U, F: FnMut(T) -> U>(self, f: F) -> Matrix<U> {
        Matrix {
            width: self.width,
            data: self.data.into_iter().map(f).collect(),
        }
    }

    pub fn as_ref(&self) -> Matrix<&T> {
        Matrix {
            width: self.width,
            data: self.data.iter().collect(),
        }
    }
}

impl<T> Index<(usize, usize)> for Matrix<T> {
    type Output = T;

    fn index(&self, (i, j): (usize, usize)) -> &Self::Output {
        self.get(i, j).as_ref().unwrap()
    }
}

impl<T> IndexMut<(usize, usize)> for Matrix<T> {
    fn index_mut(&mut self, (i, j): (usize, usize)) -> &mut Self::Output {
        self.get_mut(i, j).unwrap()
    }
}

impl<T> Index<usize> for Matrix<T> {
    type Output = [T];

    fn index(&self, index: usize) -> &Self::Output {
        &self.data[(self.width * index)..(self.width * (index+1))]
    }
}

impl<T> IndexMut<usize> for Matrix<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.data[(self.width * index)..(self.width)]
    }
}

impl<T> IntoIterator for Matrix<T> {
    type Item = T;
    type IntoIter = <Vec<T> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

impl<T> Deref for Matrix<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.data.deref()
    }
}

impl<T> Matrix<Option<T>> {
    pub fn transpose(self) -> Option<Matrix<T>> {
        if self.data.iter().all(|v| v.is_some()) {
            Some(Matrix {
                width: self.width,
                data: self.data.into_iter().map(|v| v.unwrap()).collect()
            })
        } else {
            None
        }
    }
}
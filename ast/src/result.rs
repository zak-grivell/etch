use std::collections::BTreeMap;

use crate::Span;

pub struct Results<T, E> {
    value: T,
    errors: Vec<E>,
}

impl<T, E> Results<T, E> {
    pub fn ok(value: T) -> Self {
        Self {
            value,
            errors: Vec::new(),
        }
    }

    pub fn with_error(value: T, error: E) -> Self {
        Self {
            value,
            errors: vec![error],
        }
    }

    pub fn with_errors(value: T, errors: Vec<E>) -> Self {
        Self { value, errors }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Results<U, E> {
        Results {
            value: f(self.value),
            errors: self.errors,
        }
    }

    pub fn flat_map<U>(self, f: impl FnOnce(T) -> Results<U, E>) -> Results<U, E> {
        let r = f(self.value);

        Results {
            value: r.value,
            errors: self.errors.into_iter().chain(r.errors).collect(),
        }
    }

    pub fn into_parts(self) -> (T, Vec<E>) {
        (self.value, self.errors)
    }

    pub fn zip<U>(self, other: Results<U, E>) -> Results<(T, U), E> {
        Results {
            value: (self.value, other.value),

            errors: self.errors.into_iter().chain(other.errors).collect(),
        }
    }
}
impl<T, E> FromIterator<Results<T, E>> for Results<Vec<T>, E> {
    fn from_iter<I: IntoIterator<Item = Results<T, E>>>(iter: I) -> Self {
        let mut values = Vec::new();
        let mut errors = Vec::new();

        for result in iter {
            values.push(result.value);
            errors.extend(result.errors);
        }

        Results {
            value: values,
            errors,
        }
    }
}

impl<K: Ord + Eq, V, E> FromIterator<Results<(K, V), E>> for Results<BTreeMap<K, V>, E> {
    fn from_iter<I: IntoIterator<Item = Results<(K, V), E>>>(iter: I) -> Self {
        let mut values = BTreeMap::new();
        let mut errors = Vec::new();

        for result in iter {
            let (k, v) = result.value;
            values.insert(k, v);
            errors.extend(result.errors);
        }

        Results {
            value: values,
            errors,
        }
    }
}

pub struct Diagnostic<E> {
    pub error: E,
    pub span: Span,
}

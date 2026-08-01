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

impl<C, T, E> FromIterator<Results<T, E>> for Results<C, E>
where
    C: Default + Extend<T>,
{
    fn from_iter<I: IntoIterator<Item = Results<T, E>>>(iter: I) -> Self {
        let mut collection = C::default();
        let mut errors = Vec::new();

        for item in iter {
            collection.extend(std::iter::once(item.value));
            errors.extend(item.errors);
        }

        Results {
            value: collection,
            errors,
        }
    }
}

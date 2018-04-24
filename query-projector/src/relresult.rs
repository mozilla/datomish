// Copyright 2018 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

use mentat_core::{
    Binding,
    TypedValue,
};

/// The result you get from a 'rel' query, like:
///
/// ```edn
/// [:find ?person ?name
///  :where [?person :person/name ?name]]
/// ```
///
/// There are three ways to get data out of a `RelResult`:
/// - By iterating over rows as slices. Use `result.rows()`. This is efficient and is
///   recommended in two cases:
///   1. If you don't need to take ownership of the resulting values (e.g., you're comparing
///      or making a modified clone).
///   2. When the data you're retrieving is cheap to clone. All scalar values are relatively
///      cheap: they're either small values or `Rc`.
/// - By direct reference to a row by index, using `result.row(i)`. This also returns
///   a reference.
/// - By consuming the results using `into_iter`. This allocates short-lived vectors,
///   but gives you ownership of the enclosed `TypedValue`s.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelResult<T> {
    pub width: usize,
    pub values: Vec<T>,
}

pub type StructuredRelResult = RelResult<Binding>;

impl<T> RelResult<T> {
    pub fn empty(width: usize) -> RelResult<T> {
        RelResult {
            width: width,
            values: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn row_count(&self) -> usize {
        self.values.len() / self.width
    }

    pub fn rows(&self) -> ::std::slice::Chunks<T> {
        // TODO: Nightly-only API `exact_chunks`. #47115.
        self.values.chunks(self.width)
    }

    pub fn row(&self, index: usize) -> Option<&[T]> {
        let end = self.width * (index + 1);
        if end > self.values.len() {
            None
        } else {
            let start = self.width * index;
            Some(&self.values[start..end])
        }
    }
}

#[test]
fn test_rel_result() {
    let empty = StructuredRelResult::empty(3);
    let unit = StructuredRelResult {
        width: 1,
        values: vec![TypedValue::Long(5).into()],
    };
    let two_by_two = StructuredRelResult {
        width: 2,
        values: vec![TypedValue::Long(5).into(), TypedValue::Boolean(true).into(),
                     TypedValue::Long(-2).into(), TypedValue::Boolean(false).into()],
    };

    assert!(empty.is_empty());
    assert!(!unit.is_empty());
    assert!(!two_by_two.is_empty());

    assert_eq!(empty.row_count(), 0);
    assert_eq!(unit.row_count(), 1);
    assert_eq!(two_by_two.row_count(), 2);

    assert_eq!(empty.row(0), None);
    assert_eq!(unit.row(1), None);
    assert_eq!(two_by_two.row(2), None);

    assert_eq!(unit.row(0), Some(vec![TypedValue::Long(5).into()].as_slice()));
    assert_eq!(two_by_two.row(0), Some(vec![TypedValue::Long(5).into(), TypedValue::Boolean(true).into()].as_slice()));
    assert_eq!(two_by_two.row(1), Some(vec![TypedValue::Long(-2).into(), TypedValue::Boolean(false).into()].as_slice()));

    let mut rr = two_by_two.rows();
    assert_eq!(rr.next(), Some(vec![TypedValue::Long(5).into(), TypedValue::Boolean(true).into()].as_slice()));
    assert_eq!(rr.next(), Some(vec![TypedValue::Long(-2).into(), TypedValue::Boolean(false).into()].as_slice()));
    assert_eq!(rr.next(), None);
}

// Primarily for testing.
impl From<Vec<Vec<TypedValue>>> for RelResult<Binding> {
    fn from(src: Vec<Vec<TypedValue>>) -> Self {
        if src.is_empty() {
            RelResult::empty(0)
        } else {
            let width = src.get(0).map(|r| r.len()).unwrap_or(0);
            RelResult {
                width: width,
                values: src.into_iter().flat_map(|r| r.into_iter().map(|v| v.into())).collect(),
            }
        }
    }
}

pub struct SubvecIntoIterator<T> {
    width: usize,
    values: ::std::vec::IntoIter<T>,
}

impl<T> Iterator for SubvecIntoIterator<T> {
    // TODO: this is a good opportunity to use `SmallVec` instead: most queries
    // return a handful of columns.
    type Item = Vec<T>;
    fn next(&mut self) -> Option<Self::Item> {
        let result: Vec<_> = (&mut self.values).take(self.width).collect();
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }
}

impl<T> IntoIterator for RelResult<T> {
    type Item = Vec<T>;
    type IntoIter = SubvecIntoIterator<T>;

    fn into_iter(self) -> Self::IntoIter {
        SubvecIntoIterator {
            width: self.width,
            values: self.values.into_iter(),
        }
    }
}

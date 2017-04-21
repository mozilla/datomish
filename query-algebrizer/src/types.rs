// Copyright 2016 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

use std::collections::BTreeSet;
use std::fmt::{
    Debug,
    Formatter,
    Result,
};

use enum_set::{
    CLike,
    EnumSet,
};

use mentat_core::{
    Entid,
    TypedValue,
    ValueType,
};

use mentat_query::{
    Direction,
    NamespacedKeyword,
    Order,
    Variable,
};

/// This enum models the fixed set of default tables we have -- two
/// tables and two views -- and computed tables defined in the enclosing CC.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum DatomsTable {
    Datoms,             // The non-fulltext datoms table.
    FulltextValues,     // The virtual table mapping IDs to strings.
    FulltextDatoms,     // The fulltext-datoms view.
    AllDatoms,          // Fulltext and non-fulltext datoms.
    Computed(usize),    // A computed table, tracked elsewhere in the query.
}

/// A source of rows that isn't a named table -- typically a subquery or union.
pub enum ComputedTable {
    // Subquery(BTreeSet<Variable>, ::clauses::ConjoiningClauses),
    Union {
        projection: BTreeSet<Variable>,
        type_extraction: BTreeSet<Variable>,
        arms: Vec<::clauses::ConjoiningClauses>,
    },
}

impl DatomsTable {
    pub fn name(&self) -> &'static str {
        match *self {
            DatomsTable::Datoms => "datoms",
            DatomsTable::FulltextValues => "fulltext_values",
            DatomsTable::FulltextDatoms => "fulltext_datoms",
            DatomsTable::AllDatoms => "all_datoms",
            DatomsTable::Computed(_) => "c",
        }
    }
}

pub trait ColumnName {
    fn column_name(&self) -> String;
}

/// One of the named columns of our tables.
#[derive(PartialEq, Eq, Clone)]
pub enum DatomsColumn {
    Entity,
    Attribute,
    Value,
    Tx,
    ValueTypeTag,
}

#[derive(PartialEq, Eq, Clone)]
pub enum VariableColumn {
    Variable(Variable),
    VariableTypeTag(Variable),
}

#[derive(PartialEq, Eq, Clone)]
pub enum Column {
    Fixed(DatomsColumn),
    Variable(VariableColumn),
}

impl From<DatomsColumn> for Column {
    fn from(from: DatomsColumn) -> Column {
        Column::Fixed(from)
    }
}

impl From<VariableColumn> for Column {
    fn from(from: VariableColumn) -> Column {
        Column::Variable(from)
    }
}

impl DatomsColumn {
    pub fn as_str(&self) -> &'static str {
        use self::DatomsColumn::*;
        match *self {
            Entity => "e",
            Attribute => "a",
            Value => "v",
            Tx => "tx",
            ValueTypeTag => "value_type_tag",
        }
    }
}

impl ColumnName for DatomsColumn {
    fn column_name(&self) -> String {
        self.as_str().to_string()
    }
}

impl ColumnName for VariableColumn {
    fn column_name(&self) -> String {
        match self {
            &VariableColumn::Variable(ref v) => v.to_string(),
            &VariableColumn::VariableTypeTag(ref v) => format!("{}_value_type_tag", v.as_str()),
        }
    }
}

impl Debug for VariableColumn {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            // These should agree with VariableColumn::column_name.
            &VariableColumn::Variable(ref v) => write!(f, "{}", v.as_str()),
            &VariableColumn::VariableTypeTag(ref v) => write!(f, "{}_value_type_tag", v.as_str()),
        }
    }
}

impl Debug for DatomsColumn {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{}", self.as_str())
    }
}

impl Debug for Column {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            &Column::Fixed(ref c) => c.fmt(f),
            &Column::Variable(ref v) => v.fmt(f),
        }
    }
}

/// A specific instance of a table within a query. E.g., "datoms123".
pub type TableAlias = String;

/// The association between a table and its alias. E.g., AllDatoms, "all_datoms123".
#[derive(PartialEq, Eq, Clone)]
pub struct SourceAlias(pub DatomsTable, pub TableAlias);

impl Debug for SourceAlias {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "SourceAlias({:?}, {})", self.0, self.1)
    }
}

/// A particular column of a particular aliased table. E.g., "datoms123", Attribute.
#[derive(PartialEq, Eq, Clone)]
pub struct QualifiedAlias(pub TableAlias, pub Column);

impl Debug for QualifiedAlias {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{}.{:?}", self.0, self.1)
    }
}

impl QualifiedAlias {
    pub fn new<C: Into<Column>>(table: TableAlias, column: C) -> Self {
        QualifiedAlias(table, column.into())
    }

    pub fn for_type_tag(&self) -> QualifiedAlias {
        // TODO: this only makes sense for `DatomsColumn` tables.
        QualifiedAlias(self.0.clone(), Column::Fixed(DatomsColumn::ValueTypeTag))
    }
}

#[derive(PartialEq, Eq, Clone)]
pub enum QueryValue {
    Column(QualifiedAlias),
    Entid(Entid),
    TypedValue(TypedValue),

    // This is different: a numeric value can only apply to the 'v' column, and it implicitly
    // constrains the `value_type_tag` column. For instance, a primitive long on `datoms00` of `5`
    // cannot be a boolean, so `datoms00.value_type_tag` must be in the set `#{0, 4, 5}`.
    // Note that `5 = 5.0` in SQLite, and we preserve that here.
    PrimitiveLong(i64),
}

impl Debug for QueryValue {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        use self::QueryValue::*;
        match self {
            &Column(ref qa) => {
                write!(f, "{:?}", qa)
            },
            &Entid(ref entid) => {
                write!(f, "entity({:?})", entid)
            },
            &TypedValue(ref typed_value) => {
                write!(f, "value({:?})", typed_value)
            },
            &PrimitiveLong(value) => {
                write!(f, "primitive({:?})", value)
            },

        }
    }
}

/// Represents an entry in the ORDER BY list: a variable or a variable's type tag.
/// (We require order vars to be projected, so we can simply use a variable here.)
pub struct OrderBy(pub Direction, pub VariableColumn);

impl From<Order> for OrderBy {
    fn from(item: Order) -> OrderBy {
        let Order(direction, variable) = item;
        OrderBy(direction, VariableColumn::Variable(variable))
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
/// Define the different numeric inequality operators that we support.
/// Note that we deliberately don't just use "<=" and friends as strings:
/// Datalog and SQL don't use the same operators (e.g., `<>` and `!=`).
pub enum NumericComparison {
    LessThan,
    LessThanOrEquals,
    GreaterThan,
    GreaterThanOrEquals,
    NotEquals,
}

impl NumericComparison {
    pub fn to_sql_operator(self) -> &'static str {
        use self::NumericComparison::*;
        match self {
            LessThan            => "<",
            LessThanOrEquals    => "<=",
            GreaterThan         => ">",
            GreaterThanOrEquals => ">=",
            NotEquals           => "<>",
        }
    }

    pub fn from_datalog_operator(s: &str) -> Option<NumericComparison> {
        match s {
            "<"  => Some(NumericComparison::LessThan),
            "<=" => Some(NumericComparison::LessThanOrEquals),
            ">"  => Some(NumericComparison::GreaterThan),
            ">=" => Some(NumericComparison::GreaterThanOrEquals),
            "!=" => Some(NumericComparison::NotEquals),
            _    => None,
        }
    }
}

impl Debug for NumericComparison {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        use self::NumericComparison::*;
        f.write_str(match self {
            &LessThan => "<",
            &LessThanOrEquals => "<=",
            &GreaterThan => ">",
            &GreaterThanOrEquals => ">=",
            &NotEquals => "!=",                // Datalog uses !=. SQL uses <>.
        })
    }
}

#[derive(PartialEq, Eq)]
pub enum ColumnConstraint {
    Equals(QualifiedAlias, QueryValue),
    NumericInequality {
        operator: NumericComparison,
        left: QueryValue,
        right: QueryValue,
    },
    HasType(TableAlias, ValueType),
}

#[derive(PartialEq, Eq, Debug)]
pub enum ColumnConstraintOrAlternation {
    Constraint(ColumnConstraint),
    Alternation(ColumnAlternation),
}

impl From<ColumnConstraint> for ColumnConstraintOrAlternation {
    fn from(thing: ColumnConstraint) -> Self {
        ColumnConstraintOrAlternation::Constraint(thing)
    }
}

/// A `ColumnIntersection` constraint is satisfied if all of its inner constraints are satisfied.
/// An empty intersection is always satisfied.
#[derive(PartialEq, Eq)]
pub struct ColumnIntersection(pub Vec<ColumnConstraintOrAlternation>);

impl From<Vec<ColumnConstraint>> for ColumnIntersection {
    fn from(thing: Vec<ColumnConstraint>) -> Self {
        ColumnIntersection(thing.into_iter().map(|x| x.into()).collect())
    }
}

impl Default for ColumnIntersection {
    fn default() -> Self {
        ColumnIntersection(vec![])
    }
}

impl IntoIterator for ColumnIntersection {
    type Item = ColumnConstraintOrAlternation;
    type IntoIter = ::std::vec::IntoIter<ColumnConstraintOrAlternation>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl ColumnIntersection {
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn add(&mut self, constraint: ColumnConstraintOrAlternation) {
        self.0.push(constraint);
    }

    #[inline]
    pub fn add_intersection(&mut self, constraint: ColumnConstraint) {
        self.add(ColumnConstraintOrAlternation::Constraint(constraint));
    }

    pub fn append(&mut self, other: &mut Self) {
        self.0.append(&mut other.0)
    }
}

/// A `ColumnAlternation` constraint is satisfied if at least one of its inner constraints is
/// satisfied. An empty `ColumnAlternation` is never satisfied.
#[derive(PartialEq, Eq, Debug)]
pub struct ColumnAlternation(pub Vec<ColumnIntersection>);

impl Default for ColumnAlternation {
    fn default() -> Self {
        ColumnAlternation(vec![])
    }
}

impl IntoIterator for ColumnAlternation {
    type Item = ColumnIntersection;
    type IntoIter = ::std::vec::IntoIter<ColumnIntersection>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl ColumnAlternation {
    pub fn add_alternate(&mut self, intersection: ColumnIntersection) {
        self.0.push(intersection);
    }
}

impl Debug for ColumnIntersection {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Debug for ColumnConstraint {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        use self::ColumnConstraint::*;
        match self {
            &Equals(ref qa1, ref thing) => {
                write!(f, "{:?} = {:?}", qa1, thing)
            },

            &NumericInequality { operator, ref left, ref right } => {
                write!(f, "{:?} {:?} {:?}", left, operator, right)
            },

            &HasType(ref qa, value_type) => {
                write!(f, "{:?}.value_type_tag = {:?}", qa, value_type)
            },
        }
    }
}

#[derive(PartialEq, Clone)]
pub enum EmptyBecause {
    // Var, existing, desired.
    TypeMismatch(Variable, HashSet<ValueType>, ValueType),
    NoValidTypes(Variable),
    NonNumericArgument,
    NonStringFulltextValue,
    UnresolvedIdent(NamespacedKeyword),
    InvalidAttributeIdent(NamespacedKeyword),
    InvalidAttributeEntid(Entid),
    InvalidBinding(Column, TypedValue),
    ValueTypeMismatch(ValueType, TypedValue),
    AttributeLookupFailed,         // Catch-all, because the table lookup code is lazy. TODO
}

impl Debug for EmptyBecause {
    fn fmt(&self, f: &mut Formatter) -> ::std::fmt::Result {
        use self::EmptyBecause::*;
        match self {
            &TypeMismatch(ref var, ref existing, ref desired) => {
                write!(f, "Type mismatch: {:?} can't be {:?}, because it's already {:?}",
                       var, desired, existing)
            },
            &NoValidTypes(ref var) => {
                write!(f, "Type mismatch: {:?} has no valid types", var)
            },
            &NonNumericArgument => {
                write!(f, "Non-numeric argument in numeric place")
            },
            &NonStringFulltextValue => {
                write!(f, "Non-string argument for fulltext attribute")
            },
            &UnresolvedIdent(ref kw) => {
                write!(f, "Couldn't resolve keyword {}", kw)
            },
            &InvalidAttributeIdent(ref kw) => {
                write!(f, "{} does not name an attribute", kw)
            },
            &InvalidAttributeEntid(entid) => {
                write!(f, "{} is not an attribute", entid)
            },
            &InvalidBinding(ref column, ref tv) => {
                write!(f, "{:?} cannot name column {:?}", tv, column)
            },
            &ValueTypeMismatch(value_type, ref typed_value) => {
                write!(f, "Type mismatch: {:?} doesn't match attribute type {:?}",
                       typed_value, value_type)
            },
            &AttributeLookupFailed => {
                write!(f, "Attribute lookup failed")
            },
        }
    }
}

/// A set of `ValueType`s.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueTypeSet {
    None,
    Any,
    One(ValueType),
    Many(EnumSet<ValueType>),
}

impl Default for ValueTypeSet {
    fn default() -> ValueTypeSet {
        ValueTypeSet::Any
    }
}

impl ValueTypeSet {
    /// Return a set containing only `t`.
    pub fn unit(t: ValueType) -> ValueTypeSet {
        ValueTypeSet::One(t)
    }

    /// Return a set containing `Double` and `Long`.
    pub fn of_numeric_types() -> ValueTypeSet {
        let mut numeric_types = EnumSet::<ValueType>::new();
        numeric_types.insert(ValueType::Double);
        numeric_types.insert(ValueType::Long);
        ValueTypeSet::Many(numeric_types)
    }
}

trait EnumSetExtensions<T: CLike + Clone> {
    /// Return a set containing both `x` and `y`.
    fn both(x: T, y: T) -> EnumSet<T>;

    /// Return a clone of `self` with `y` added.
    fn with(&self, y: T) -> EnumSet<T>;
}

impl<T: CLike + Clone> EnumSetExtensions<T> for EnumSet<T> {
    /// Return a set containing both `x` and `y`.
    fn both(x: T, y: T) -> Self {
        let mut o = EnumSet::new();
        o.insert(x);
        o.insert(y);
        o
    }

    /// Return a clone of `self` with `y` added.
    fn with(&self, y: T) -> EnumSet<T> {
        let mut o = self.clone();
        o.insert(y);
        o
    }
}

impl ValueTypeSet {
    /// Returns a set containing all the types in this set and `other`.
    pub fn union(&self, other: &ValueTypeSet) -> ValueTypeSet {
        match self {
            &ValueTypeSet::None => other.clone(),
            &ValueTypeSet::Any => ValueTypeSet::Any,
            &ValueTypeSet::One(t) if other.contains(t) => other.clone(),
            &ValueTypeSet::One(t) => {
                match other {
                    &ValueTypeSet::None => self.clone(),
                    &ValueTypeSet::Any => ValueTypeSet::Any,
                    &ValueTypeSet::One(o) if t == o => self.clone(),
                    &ValueTypeSet::One(o) => ValueTypeSet::Many(EnumSet::both(o, t)),
                    &ValueTypeSet::Many(os) => ValueTypeSet::Many(os.with(t)),
                }
            },
            &ValueTypeSet::Many(ts) => {
                match other {
                    &ValueTypeSet::None => self.clone(),
                    &ValueTypeSet::Any => ValueTypeSet::Any,
                    &ValueTypeSet::One(o) if ts.contains(&o) => self.clone(),
                    &ValueTypeSet::One(o) => ValueTypeSet::Many(ts.with(o)),
                    &ValueTypeSet::Many(os) => ValueTypeSet::Many(os.union(ts)),
                }
            },
        }
    }

    pub fn intersection(&self, other: &ValueTypeSet) -> ValueTypeSet {
        match (self, other) {
            (&ValueTypeSet::None, _) => ValueTypeSet::None,
            (_, &ValueTypeSet::None) => ValueTypeSet::None,
            (s, &ValueTypeSet::Any) => s.clone(),
            (&ValueTypeSet::Any, s) => s.clone(),
            (&ValueTypeSet::One(t), s) => if s.contains(t) { ValueTypeSet::One(t) } else { ValueTypeSet::None },
            (s, &ValueTypeSet::One(t)) => if s.contains(t) { ValueTypeSet::One(t) } else { ValueTypeSet::None },
            (&ValueTypeSet::Many(ref left), &ValueTypeSet::Many(ref right)) => {
                let result: EnumSet<ValueType> = left.intersection(right.clone());
                match result.len() {
                    0 => ValueTypeSet::None,
                    1 => ValueTypeSet::One(result.into_iter().next().unwrap()),
                    _ => ValueTypeSet::Many(result),
                }
            },
        }
    }

    /// Return an arbitrary type that's part of this set.
    pub fn exemplar(&self) -> Option<ValueType> {
        match self {
            &ValueTypeSet::None => None,
            &ValueTypeSet::Any => Some(ValueType::Ref),
            &ValueTypeSet::One(t) => Some(t),
            &ValueTypeSet::Many(ref s) => s.iter().next(),
        }
    }

    pub fn contains(&self, vt: ValueType) -> bool {
        match self {
            &ValueTypeSet::None => false,
            &ValueTypeSet::Any => true,
            &ValueTypeSet::One(t) => t == vt,
            &ValueTypeSet::Many(ref s) => s.contains(&vt),
        }
    }

    pub fn is_none(&self) -> bool {
        match self {
            &ValueTypeSet::None => true,
            _ => false,
        }
    }

    pub fn is_unit(&self) -> bool {
        match self {
            &ValueTypeSet::One(_) => true,
            _ => false,
        }
    }
}
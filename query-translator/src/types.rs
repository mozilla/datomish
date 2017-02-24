// Copyright 2016 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

#![allow(dead_code, unused_imports)]

use mentat_core::{
    Entid,
    TypedValue,
};

use mentat_query_algebrizer::{
    AlgebraicQuery,
    ConjoiningClauses,
    DatomsColumn,
    DatomsTable,
    QualifiedAlias,
    SourceAlias,
};

use mentat_sql;

use mentat_sql::{
    BuildQueryResult,
    QueryBuilder,
    QueryFragment,
    SQLiteQueryBuilder,
    SQLQuery,
};

//---------------------------------------------------------
// A Mentat-focused representation of a SQL query.

/// One of the things that can appear in a projection or a constraint. Note that we use
/// `TypedValue` here; it's not pure SQL, but it avoids us having to concern ourselves at this
/// point with the translation between a `TypedValue` and the storage-layer representation.
///
/// Eventually we might allow different translations by providing a different `QueryBuilder`
/// implementation for each storage backend. Passing `TypedValue`s here allows for that.
pub enum ColumnOrExpression {
    Column(QualifiedAlias),
    Entid(Entid),       // Because it's so common.
    Value(TypedValue),
}

pub type Name = String;

pub struct ProjectedColumn(pub ColumnOrExpression, pub Name);

pub enum Projection {
    Columns(Vec<ProjectedColumn>),
    Star,
    One,
}

#[derive(Copy, Clone)]
pub struct Op(&'static str);      // TODO: we can do better than this!

pub enum Constraint {
    Infix {
        op: Op,
        left: ColumnOrExpression,
        right: ColumnOrExpression
    }
}

impl Constraint {
    pub fn equal(left: ColumnOrExpression, right: ColumnOrExpression) -> Constraint {
        Constraint::Infix {
            op: Op("="),
            left: left,
            right: right,
        }
    }
}

enum JoinOp {
    Inner,
}

// Short-hand for a list of tables all inner-joined.
pub struct TableList(pub Vec<SourceAlias>);

pub struct Join {
    left: TableOrSubquery,
    op: JoinOp,
    right: TableOrSubquery,
    // TODO: constraints (ON, USING).
}

enum TableOrSubquery {
    Table(SourceAlias),
    // TODO: Subquery.
}

pub enum FromClause {
    TableList(TableList),      // Short-hand for a pile of inner joins.
    Join(Join),
}

pub struct SelectQuery {
    pub projection: Projection,
    pub from: FromClause,
    pub constraints: Vec<Constraint>,
}

// We know that DatomsColumns are safe to serialize.
fn push_column(qb: &mut QueryBuilder, col: &DatomsColumn) {
    qb.push_sql(col.as_str());
}

//---------------------------------------------------------
// Turn that representation into SQL.

impl QueryFragment for ColumnOrExpression {
    fn push_sql(&self, out: &mut QueryBuilder) -> BuildQueryResult {
        use self::ColumnOrExpression::*;
        match self {
            &Column(QualifiedAlias(ref table, ref column)) => {
                out.push_identifier(table.as_str())?;
                out.push_sql(".");
                push_column(out, column);
                Ok(())
            },
            &Entid(entid) => {
                out.push_sql(entid.to_string().as_str());
                Ok(())
            },
            &Value(ref v) => {
                out.push_typed_value(v)
            },
        }
    }
}

impl QueryFragment for Projection {
    fn push_sql(&self, out: &mut QueryBuilder) -> BuildQueryResult {
        use self::Projection::*;
        match self {
            &One => out.push_sql("1"),
            &Star => out.push_sql("*"),
            &Columns(ref cols) => {
                let &ProjectedColumn(ref col, ref alias) = &cols[0];
                col.push_sql(out)?;
                out.push_sql(" AS ");
                out.push_identifier(alias.as_str())?;

                for &ProjectedColumn(ref col, ref alias) in &cols[1..] {
                    out.push_sql(", ");
                    col.push_sql(out)?;
                    out.push_sql(" AS ");
                    out.push_identifier(alias.as_str())?;
                }
            },
        };
        Ok(())
    }
}

impl QueryFragment for Op {
    fn push_sql(&self, out: &mut QueryBuilder) -> BuildQueryResult {
        // No escaping needed.
        out.push_sql(self.0);
        Ok(())
    }
}

impl QueryFragment for Constraint {
    fn push_sql(&self, out: &mut QueryBuilder) -> BuildQueryResult {
        use self::Constraint::*;
        match self {
            &Infix { ref op, ref left, ref right } => {
                left.push_sql(out)?;
                out.push_sql(" ");
                op.push_sql(out)?;
                out.push_sql(" ");
                right.push_sql(out)
            }
        }
    }
}

impl QueryFragment for JoinOp {
    fn push_sql(&self, out: &mut QueryBuilder) -> BuildQueryResult {
        out.push_sql(" JOIN ");
        Ok(())
    }
}

// We don't own SourceAlias or QueryFragment, so we can't implement the trait.
fn source_alias_push_sql(out: &mut QueryBuilder, sa: &SourceAlias) -> BuildQueryResult {
    let &SourceAlias(ref table, ref alias) = sa;
    out.push_identifier(table.name())?;
    out.push_sql(" AS ");
    out.push_identifier(alias.as_str())
}

impl QueryFragment for TableList {
    fn push_sql(&self, out: &mut QueryBuilder) -> BuildQueryResult {
        if self.0.is_empty() {
            return Ok(());
        }

        source_alias_push_sql(out, &self.0[0])?;

        for sa in self.0.iter().skip(1) {
            out.push_sql(", ");
            source_alias_push_sql(out, sa)?;
        }
        Ok(())
    }
}

impl QueryFragment for Join {
    fn push_sql(&self, out: &mut QueryBuilder) -> BuildQueryResult {
        self.left.push_sql(out)?;
        self.op.push_sql(out)?;
        self.right.push_sql(out)
    }
}

impl QueryFragment for TableOrSubquery {
    fn push_sql(&self, out: &mut QueryBuilder) -> BuildQueryResult {
        use self::TableOrSubquery::*;
        match self {
            &Table(ref sa) => source_alias_push_sql(out, sa)
        }
    }
}

impl QueryFragment for FromClause {
    fn push_sql(&self, out: &mut QueryBuilder) -> BuildQueryResult {
        use self::FromClause::*;
        match self {
            &TableList(ref table_list) => table_list.push_sql(out),
            &Join(ref join) => join.push_sql(out),
        }
    }
}

impl QueryFragment for SelectQuery {
    fn push_sql(&self, out: &mut QueryBuilder) -> BuildQueryResult {
        out.push_sql("SELECT ");
        self.projection.push_sql(out)?;

        out.push_sql(" FROM ");
        self.from.push_sql(out)?;

        if self.constraints.is_empty() {
            return Ok(());
        }

        out.push_sql(" WHERE ");
        self.constraints[0].push_sql(out)?;

        for constraint in self.constraints[1..].iter() {
            out.push_sql(" AND ");
            constraint.push_sql(out)?;
        }

        Ok(())
    }
}

impl SelectQuery {
    pub fn to_sql_query(&self) -> mentat_sql::Result<SQLQuery> {
        let mut builder = SQLiteQueryBuilder::new();
        self.push_sql(&mut builder).map(|_| builder.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_end_to_end() {

        // [:find ?x :where [?x 65537 ?v] [?x 65536 ?v]]
        let datoms00 = "datoms00".to_string();
        let datoms01 = "datoms01".to_string();
        let eq = Op("=");
        let source_aliases = vec![
            SourceAlias(DatomsTable::Datoms, datoms00.clone()),
            SourceAlias(DatomsTable::Datoms, datoms01.clone()),
        ];
        let query = SelectQuery {
            projection: Projection::Columns(
                            vec![
                                ProjectedColumn(
                                    ColumnOrExpression::Column(QualifiedAlias(datoms00.clone(), DatomsColumn::Entity)),
                                    "x".to_string()),
                            ]),
            from: FromClause::TableList(TableList(source_aliases)),
            constraints: vec![
                Constraint::Infix {
                    op: eq.clone(),
                    left: ColumnOrExpression::Column(QualifiedAlias(datoms01.clone(), DatomsColumn::Value)),
                    right: ColumnOrExpression::Column(QualifiedAlias(datoms00.clone(), DatomsColumn::Value)),
                },
                Constraint::Infix {
                    op: eq.clone(),
                    left: ColumnOrExpression::Column(QualifiedAlias(datoms00.clone(), DatomsColumn::Attribute)),
                    right: ColumnOrExpression::Entid(65537),
                },
                Constraint::Infix {
                    op: eq.clone(),
                    left: ColumnOrExpression::Column(QualifiedAlias(datoms01.clone(), DatomsColumn::Attribute)),
                    right: ColumnOrExpression::Entid(65536),
                },
            ],
        };

        let SQLQuery { sql, args } = query.to_sql_query().unwrap();
        println!("{}", sql);
        assert_eq!("SELECT `datoms00`.e AS `x` FROM `datoms` AS `datoms00`, `datoms` AS `datoms01` WHERE `datoms01`.v = `datoms00`.v AND `datoms00`.a = 65537 AND `datoms01`.a = 65536", sql);
        assert!(args.is_empty());
    }
}

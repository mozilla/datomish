// Copyright 2016 Mozilla
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

extern crate combine;
extern crate edn;
extern crate mentat_parser_utils;
extern crate mentat_query;

use std; // To refer to std::result::Result.

use std::collections::BTreeSet;

use self::combine::{eof, many, many1, optional, parser, satisfy, satisfy_map, Parser, ParseResult, Stream};
use self::combine::combinator::{any, choice, or, try};

use self::mentat_parser_utils::{
    KeywordMapParser,
    ResultParser,
    ValueParseError,
};

use self::mentat_parser_utils::value_and_span::Stream as ValueStream;
use self::mentat_parser_utils::value_and_span::{
    Item,
    OfExactlyParsing,
    keyword_map,
    list,
    map,
    seq,
    vector,
};

use self::mentat_query::{
    Direction,
    Element,
    FindQuery,
    FindSpec,
    FnArg,
    FromValue,
    Limit,
    Order,
    OrJoin,
    OrWhereClause,
    NotJoin,
    Pattern,
    PatternNonValuePlace,
    PatternValuePlace,
    Predicate,
    PredicateFn,
    SrcVar,
    UnifyVars,
    Variable,
    WhereClause,
};

error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    foreign_links {
        EdnParseError(edn::parse::ParseError);
    }

    errors {
        DuplicateVariableError {
            description("duplicate variable")
            display("duplicates in variable list")
        }

        NotAVariableError(value: edn::ValueAndSpan) {
            description("not a variable")
            display("not a variable: '{}'", value)
        }

        FindParseError(e: ValueParseError) {
            description(":find parse error")
            display(":find parse error")
        }

        WhereParseError(e: ValueParseError) {
            description(":where parse error")
            display(":where parse error")
        }

        // Not yet used.
        WithParseError {
            description(":with parse error")
            display(":with parse error")
        }

        InvalidInputError(value: edn::Value) {
            description("invalid input")
            display("invalid input: '{}'", value)
        }

        MissingFieldError(value: edn::Keyword) {
            description("missing field")
            display("missing field: '{}'", value)
        }

        UnknownLimitVar(var: edn::PlainSymbol) {
            description("limit var not present in :in")
            display("limit var {} not present in :in", var)
        }

        InvalidLimit(val: edn::Value) {
            description("limit value not valid")
            display("expected natural number, got {}", val)
        }
    }
}

pub struct Query<'a>(std::marker::PhantomData<&'a ()>);

def_parser!(Query, variable, Variable, {
    satisfy_map(Variable::from_value)
});

def_parser!(Query, source_var, SrcVar, {
    satisfy_map(SrcVar::from_value)
});

// TODO: interning.
def_parser!(Query, predicate_fn, PredicateFn, {
    satisfy_map(PredicateFn::from_value)
});

def_parser!(Query, fn_arg, FnArg, {
    satisfy_map(FnArg::from_value).or(vector().of_exactly(many::<Vec<FnArg>, _>(Query::fn_arg())).map(FnArg::Vector))
});

def_parser!(Query, arguments, Vec<FnArg>, {
    (many::<Vec<FnArg>, _>(Query::fn_arg()))
});

def_parser!(Query, direction, Direction, {
    satisfy_map(|v: &edn::ValueAndSpan| {
        match v.inner {
            edn::SpannedValue::PlainSymbol(ref s) => {
                let name = s.0.as_str();
                match name {
                    "asc" => Some(Direction::Ascending),
                    "desc" => Some(Direction::Descending),
                    _ => None,
                }
            },
            _ => None,
        }
    })
});

def_parser!(Query, order, Order, {
    seq().of_exactly((Query::direction(), Query::variable()))
         .map(|(d, v)| Order(d, v))
         .or(Query::variable().map(|v| Order(Direction::Ascending, v)))
});

pub struct Where<'a>(std::marker::PhantomData<&'a ()>);

def_parser!(Where, pattern_value_place, PatternValuePlace,  {
    satisfy_map(PatternValuePlace::from_value)
});

def_parser!(Query, natural_number, u64, {
    any().and_then(|v: &edn::ValueAndSpan| {
        match v.inner {
            edn::SpannedValue::Integer(x) if (x > 0) => {
                Ok(x as u64)
            },
            ref spanned => {
                let e = Box::new(Error::from_kind(ErrorKind::InvalidLimit(spanned.clone().into())));
                Err(combine::primitives::Error::Other(e))
            },
        }
    })
});

def_parser!(Where, pattern_non_value_place, PatternNonValuePlace, {
    satisfy_map(PatternNonValuePlace::from_value)
});

def_matches_plain_symbol!(Where, and, "and");

def_matches_plain_symbol!(Where, or, "or");

def_matches_plain_symbol!(Where, or_join, "or-join");

def_matches_plain_symbol!(Where, not, "not");

def_matches_plain_symbol!(Where, not_join, "not-join");

def_parser!(Where, rule_vars, Vec<Variable>, {
    seq()
        .of_exactly(many1(Query::variable()))
});

def_parser!(Where, or_pattern_clause, OrWhereClause, {
    Where::clause().map(|clause| OrWhereClause::Clause(clause))
});

def_parser!(Where, or_and_clause, OrWhereClause, {
    seq()
        .of_exactly(Where::and()
            .with(many1(Where::clause()))
            .map(OrWhereClause::And))
});

def_parser!(Where, or_where_clause, OrWhereClause, {
    choice([Where::or_pattern_clause(), Where::or_and_clause()])
});

def_parser!(Where, or_clause, WhereClause, {
    seq()
        .of_exactly(Where::or()
            .with(many1(Where::or_where_clause()))
            .map(|clauses| {
                WhereClause::OrJoin(OrJoin::new(UnifyVars::Implicit, clauses))
            }))
});

def_parser!(Where, or_join_clause, WhereClause, {
    seq()
        .of_exactly(Where::or_join()
            .with(Where::rule_vars())
            .and(many1(Where::or_where_clause()))
            .map(|(vars, clauses)| {
                WhereClause::OrJoin(OrJoin::new(UnifyVars::Explicit(vars), clauses))
            }))
});

def_parser!(Where, not_clause, WhereClause, {
    seq()
        .of_exactly(Where::not()
            .with(many1(Where::clause()))
            .map(|clauses| {
                WhereClause::NotJoin(
                    NotJoin {
                        unify_vars: UnifyVars::Implicit,
                        clauses: clauses,
                    })
            }))
});

def_parser!(Where, not_join_clause, WhereClause, {
    seq()
        .of_exactly(Where::not_join()
            .with(Where::rule_vars())
            .and(many1(Where::clause()))
            .map(|(vars, clauses)| {
                WhereClause::NotJoin(
                    NotJoin {
                        unify_vars: UnifyVars::Explicit(vars),
                        clauses: clauses,
                    })
            }))
});

/// A vector containing just a parenthesized filter expression.
def_parser!(Where, pred, WhereClause, {
    // Accept either a nested list or a nested vector here:
    // `[(foo ?x ?y)]` or `[[foo ?x ?y]]`
    vector()
        .of_exactly(seq()
            .of_exactly((Query::predicate_fn(), Query::arguments())
                .map(|(f, args)| {
                    WhereClause::Pred(
                        Predicate {
                            operator: f.0,
                            args: args,
                        })
                })))
});

def_parser!(Where, pattern, WhereClause, {
    vector()
        .of_exactly(
            // While *technically* Datomic allows you to have a query like:
            // [:find … :where [[?x]]]
            // We don't -- we require at least e, a.
            (optional(Query::source_var()),              // src
             Where::pattern_non_value_place(),           // e
             Where::pattern_non_value_place(),           // a
             optional(Where::pattern_value_place()),     // v
             optional(Where::pattern_non_value_place())) // tx
                .and_then(|(src, e, a, v, tx)| {
                    let v = v.unwrap_or(PatternValuePlace::Placeholder);
                    let tx = tx.unwrap_or(PatternNonValuePlace::Placeholder);

                    // Pattern::new takes care of reversal of reversed
                    // attributes: [?x :foo/_bar ?y] turns into
                    // [?y :foo/bar ?x].
                    //
                    // This is a bit messy: the inner conversion to a Pattern can
                    // fail if the input is something like
                    //
                    // ```edn
                    // [?x :foo/_reversed 23.4]
                    // ```
                    //
                    // because
                    //
                    // ```edn
                    // [23.4 :foo/reversed ?x]
                    // ```
                    //
                    // is nonsense. That leaves us with a nested optional, which we unwrap here.
                    Pattern::new(src, e, a, v, tx)
                        .map(WhereClause::Pattern)
                        .ok_or(combine::primitives::Error::Expected("pattern".into()))
                }))
});

def_parser!(Where, clause, WhereClause, {
    choice([try(Where::pattern()),
            // It's either
            //   (or-join [vars] clauses…)
            // or
            //   (or clauses…)
            // We don't yet handle source vars.
            try(Where::or_join_clause()),
            try(Where::or_clause()),
            try(Where::not_join_clause()),
            try(Where::not_clause()),

            try(Where::pred()),
    ])
});

def_parser!(Where, clauses, Vec<WhereClause>, {
    // Right now we only support patterns and predicates. See #239 for more.
    (many1::<Vec<WhereClause>, _>(Where::clause()))
});

pub struct Find<'a>(std::marker::PhantomData<&'a ()>);

def_matches_plain_symbol!(Find, period, ".");

def_matches_plain_symbol!(Find, ellipsis, "...");

def_parser!(Find, find_scalar, FindSpec, {
    Query::variable()
        .skip(Find::period())
        .map(|var| FindSpec::FindScalar(Element::Variable(var)))
});

def_parser!(Find, find_coll, FindSpec, {
    vector()
        .of_exactly(Query::variable()
            .skip(Find::ellipsis()))
            .map(|var| FindSpec::FindColl(Element::Variable(var)))
});

def_parser!(Find, elements, Vec<Element>, {
    many1::<Vec<Element>, _>(Query::variable().map(Element::Variable))
});

def_parser!(Find, find_rel, FindSpec, {
    Find::elements().map(FindSpec::FindRel)
});

def_parser!(Find, find_tuple, FindSpec, {
    vector()
        .of_exactly(Find::elements().map(FindSpec::FindTuple))
});

/// Parse a stream of values into one of four find specs.
///
/// `:find` must be an array of plain var symbols (?foo), pull expressions, and aggregates.  For now
/// we only support variables and the annotations necessary to declare which flavor of :find we
/// want:
///
///
///     `?x ?y ?z  `     = FindRel
///     `[?x ...]  `     = FindColl
///     `?x .      `     = FindScalar
///     `[?x ?y ?z]`     = FindTuple
def_parser!(Find, spec, FindSpec, {
    // Any one of the four specs might apply, so we combine them with `choice`.  Our parsers consume
    // input, so we need to wrap them in `try` so that they operate independently.
    choice::<[&mut Parser<Input = _, Output = FindSpec>; 4], _>
        ([&mut try(Find::find_scalar()),
          &mut try(Find::find_coll()),
          &mut try(Find::find_tuple()),
          &mut try(Find::find_rel())])
});

def_parser!(Find, vars, BTreeSet<Variable>, {
    many(Query::variable()).and_then(|vars: Vec<Variable>| {
        let given = vars.len();
        let set: BTreeSet<Variable> = vars.into_iter().collect();
        if given != set.len() {
            // TODO: find out what the variable is!
            let e = Box::new(Error::from_kind(ErrorKind::DuplicateVariableError));
            Err(combine::primitives::Error::Other(e))
        } else {
            Ok(set)
        }
    })
});

/// This is awkward, but will do for now.  We use `keyword_map()` to optionally accept vector find
/// queries, then we use `FindQueryPart` to collect parts that have heterogeneous types; and then we
/// construct a `FindQuery` from them.
def_parser!(Find, query, FindQuery, {
    let find_map = keyword_map_of!(
        ("find", Find::spec()),
        ("in", Find::vars()),
        ("limit", Query::variable().map(Limit::Variable).or(Query::natural_number().map(Limit::Fixed))),
        ("order", many1(Query::order())),
        ("where", Where::clauses()),
        ("with", Find::vars()) // Note: no trailing comma allowed!
    );

    (or(keyword_map(), vector()))
        .of_exactly(find_map)
        .and_then(|(find_spec, in_vars, limit, order_clauses, where_clauses, with_vars) | -> std::result::Result<FindQuery, combine::primitives::Error<&edn::ValueAndSpan, &edn::ValueAndSpan>>  {
            let limit = limit.unwrap_or(Limit::None);

            // Make sure that if we have `:limit ?x`, `?x` appears in `:in`.
            let in_vars = in_vars.unwrap_or(BTreeSet::default());
            if let Limit::Variable(ref v) = limit {
                if !in_vars.contains(v) {
                    let e = Box::new(Error::from_kind(ErrorKind::UnknownLimitVar(v.name())));
                    return Err(combine::primitives::Error::Other(e));
                }
            }

            Ok(FindQuery {
                default_source: SrcVar::DefaultSrc,
                find_spec: find_spec.clone().ok_or(combine::primitives::Error::Unexpected("expected :find".into()))?,
                in_sources: BTreeSet::default(),    // TODO
                in_vars: in_vars,
                limit: limit,
                order: order_clauses,
                where_clauses: where_clauses.ok_or(combine::primitives::Error::Unexpected("expected :where".into()))?,
                with: with_vars.unwrap_or(BTreeSet::default()),
            })
        })
});

pub fn parse_find_string(string: &str) -> Result<FindQuery> {
    let expr = edn::parse::value(string)?;
    Find::query()
        .parse(expr.atom_stream())
        .map(|x| x.0)
        .map_err(|e| Error::from_kind(ErrorKind::FindParseError(e.into())))
}

#[cfg(test)]
mod test {
    extern crate combine;
    extern crate edn;
    extern crate mentat_query;

    use std::rc::Rc;

    use self::combine::Parser;
    use self::edn::OrderedFloat;
    use self::mentat_query::{
        Element,
        FindSpec,
        NonIntegerConstant,
        Pattern,
        PatternNonValuePlace,
        PatternValuePlace,
        SrcVar,
        Variable,
    };

    use super::*;

    fn variable(x: edn::PlainSymbol) -> Variable {
        Variable(Rc::new(x))
    }

    fn ident_kw(kw: edn::NamespacedKeyword) -> PatternNonValuePlace {
        PatternNonValuePlace::Ident(Rc::new(kw))
    }

    fn ident(ns: &str, name: &str) -> PatternNonValuePlace {
        ident_kw(edn::NamespacedKeyword::new(ns, name))
    }

    #[test]
    fn test_pattern_mixed() {
        let e = edn::PlainSymbol::new("_");
        let a = edn::NamespacedKeyword::new("foo", "bar");
        let v = OrderedFloat(99.9);
        let tx = edn::PlainSymbol::new("?tx");
        let input = edn::Value::Vector(vec!(edn::Value::PlainSymbol(e.clone()),
                                            edn::Value::NamespacedKeyword(a.clone()),
                                            edn::Value::Float(v.clone()),
                                            edn::Value::PlainSymbol(tx.clone())));
        assert_parses_to!(Where::pattern, input, WhereClause::Pattern(Pattern {
            source: None,
            entity: PatternNonValuePlace::Placeholder,
            attribute: ident_kw(a),
            value: PatternValuePlace::Constant(NonIntegerConstant::Float(v)),
            tx: PatternNonValuePlace::Variable(variable(tx)),
        }));
    }

    #[test]
    fn test_pattern_vars() {
        let s = edn::PlainSymbol::new("$x");
        let e = edn::PlainSymbol::new("?e");
        let a = edn::PlainSymbol::new("?a");
        let v = edn::PlainSymbol::new("?v");
        let tx = edn::PlainSymbol::new("?tx");
        let input = edn::Value::Vector(vec!(edn::Value::PlainSymbol(s.clone()),
                 edn::Value::PlainSymbol(e.clone()),
                 edn::Value::PlainSymbol(a.clone()),
                 edn::Value::PlainSymbol(v.clone()),
                 edn::Value::PlainSymbol(tx.clone())));
        assert_parses_to!(Where::pattern, input, WhereClause::Pattern(Pattern {
            source: Some(SrcVar::NamedSrc("x".to_string())),
            entity: PatternNonValuePlace::Variable(variable(e)),
            attribute: PatternNonValuePlace::Variable(variable(a)),
            value: PatternValuePlace::Variable(variable(v)),
            tx: PatternNonValuePlace::Variable(variable(tx)),
        }));
    }

    #[test]
    fn test_pattern_reversed_invalid() {
        let e = edn::PlainSymbol::new("_");
        let a = edn::NamespacedKeyword::new("foo", "_bar");
        let v = OrderedFloat(99.9);
        let tx = edn::PlainSymbol::new("?tx");
        let input = edn::Value::Vector(vec!(edn::Value::PlainSymbol(e.clone()),
                 edn::Value::NamespacedKeyword(a.clone()),
                 edn::Value::Float(v.clone()),
                 edn::Value::PlainSymbol(tx.clone())));

        let input = input.with_spans();
        let mut par = Where::pattern();
        let result = par.parse(input.atom_stream());
        assert!(matches!(result, Err(_)), "Expected a parse error.");
    }

    #[test]
    fn test_pattern_reversed() {
        let e = edn::PlainSymbol::new("_");
        let a = edn::NamespacedKeyword::new("foo", "_bar");
        let v = edn::PlainSymbol::new("?v");
        let tx = edn::PlainSymbol::new("?tx");
        let input = edn::Value::Vector(vec!(edn::Value::PlainSymbol(e.clone()),
                 edn::Value::NamespacedKeyword(a.clone()),
                 edn::Value::PlainSymbol(v.clone()),
                 edn::Value::PlainSymbol(tx.clone())));

        // Note that the attribute is no longer reversed, and the entity and value have
        // switched places.
        assert_parses_to!(Where::pattern, input, WhereClause::Pattern(Pattern {
            source: None,
            entity: PatternNonValuePlace::Variable(variable(v)),
            attribute: ident("foo", "bar"),
            value: PatternValuePlace::Placeholder,
            tx: PatternNonValuePlace::Variable(variable(tx)),
        }));
    }

    #[test]
    fn test_rule_vars() {
        let e = edn::PlainSymbol::new("?e");
        let input = edn::Value::Vector(vec![edn::Value::PlainSymbol(e.clone())]);
        assert_parses_to!(Where::rule_vars, input,
                          vec![variable(e.clone())]);
    }

    #[test]
    fn test_repeated_vars() {
        let e = edn::PlainSymbol::new("?e");
        let f = edn::PlainSymbol::new("?f");
        let input = edn::Value::Vector(vec![edn::Value::PlainSymbol(e.clone()),
                                            edn::Value::PlainSymbol(f.clone()),]);
        assert_parses_to!(|| vector().of_exactly(Find::vars()), input,
                          vec![variable(e.clone()), variable(f.clone())].into_iter().collect());

        let g = edn::PlainSymbol::new("?g");
        let input = edn::Value::Vector(vec![edn::Value::PlainSymbol(g.clone()),
                                            edn::Value::PlainSymbol(g.clone()),]);

        let input = input.with_spans();
        let mut par = vector().of_exactly(Find::vars());
        let result = par.parse(input.atom_stream())
            .map(|x| x.0)
            .map_err(|e| if let Some(combine::primitives::Error::Other(x)) = e.errors.into_iter().next() {
                // Pattern matching on boxes is rocket science until Rust Nightly features hit
                // stable.  ErrorKind isn't Clone, so convert to strings.  We could pattern match
                // for exact comparison here.
                x.downcast::<Error>().ok().map(|e| e.to_string())
            } else {
                None
            });
        assert_eq!(result, Err(Some("duplicates in variable list".to_string())));
    }

    #[test]
    fn test_or() {
        let oj = edn::PlainSymbol::new("or");
        let e = edn::PlainSymbol::new("?e");
        let a = edn::PlainSymbol::new("?a");
        let v = edn::PlainSymbol::new("?v");
        let input = edn::Value::List(
            vec![edn::Value::PlainSymbol(oj),
                 edn::Value::Vector(vec![edn::Value::PlainSymbol(e.clone()),
                                         edn::Value::PlainSymbol(a.clone()),
                                         edn::Value::PlainSymbol(v.clone())])].into_iter().collect());
        assert_parses_to!(Where::or_clause, input,
                          WhereClause::OrJoin(
                              OrJoin::new(UnifyVars::Implicit,
                                          vec![OrWhereClause::Clause(
                                              WhereClause::Pattern(Pattern {
                                                  source: None,
                                                  entity: PatternNonValuePlace::Variable(variable(e)),
                                                  attribute: PatternNonValuePlace::Variable(variable(a)),
                                                  value: PatternValuePlace::Variable(variable(v)),
                                                  tx: PatternNonValuePlace::Placeholder,
                                              }))])));
    }

    #[test]
    fn test_or_join() {
        let oj = edn::PlainSymbol::new("or-join");
        let e = edn::PlainSymbol::new("?e");
        let a = edn::PlainSymbol::new("?a");
        let v = edn::PlainSymbol::new("?v");
        let input = edn::Value::List(
            vec![edn::Value::PlainSymbol(oj),
                 edn::Value::Vector(vec![edn::Value::PlainSymbol(e.clone())]),
                 edn::Value::Vector(vec![edn::Value::PlainSymbol(e.clone()),
                                         edn::Value::PlainSymbol(a.clone()),
                                         edn::Value::PlainSymbol(v.clone())])].into_iter().collect());
        assert_parses_to!(Where::or_join_clause, input,
                          WhereClause::OrJoin(
                              OrJoin::new(UnifyVars::Explicit(vec![variable(e.clone())]),
                                          vec![OrWhereClause::Clause(
                                              WhereClause::Pattern(Pattern {
                                                  source: None,
                                                  entity: PatternNonValuePlace::Variable(variable(e)),
                                                  attribute: PatternNonValuePlace::Variable(variable(a)),
                                                  value: PatternValuePlace::Variable(variable(v)),
                                                  tx: PatternNonValuePlace::Placeholder,
                                              }))])));
    }

    #[test]
    fn test_not() {
        let e = edn::PlainSymbol::new("?e");
        let a = edn::PlainSymbol::new("?a");
        let v = edn::PlainSymbol::new("?v");

        assert_edn_parses_to!(Where::not_clause,
                              "(not [?e ?a ?v])",
                              WhereClause::NotJoin(
                              NotJoin {
                                  unify_vars: UnifyVars::Implicit,
                                  clauses: vec![
                                      WhereClause::Pattern(Pattern {
                                          source: None,
                                          entity: PatternNonValuePlace::Variable(variable(e)),
                                          attribute: PatternNonValuePlace::Variable(variable(a)),
                                          value: PatternValuePlace::Variable(variable(v)),
                                          tx: PatternNonValuePlace::Placeholder,
                                      })],
                              }));
    }

    #[test]
    fn test_not_join() {
        let e = edn::PlainSymbol::new("?e");
        let a = edn::PlainSymbol::new("?a");
        let v = edn::PlainSymbol::new("?v");
        
        assert_edn_parses_to!(Where::not_join_clause,
                              "(not-join [?e] [?e ?a ?v])",
                              WhereClause::NotJoin(
                              NotJoin {
                                  unify_vars: UnifyVars::Explicit(vec![variable(e.clone())]),
                                  clauses: vec![WhereClause::Pattern(Pattern {
                                          source: None,
                                          entity: PatternNonValuePlace::Variable(variable(e)),
                                          attribute: PatternNonValuePlace::Variable(variable(a)),
                                          value: PatternValuePlace::Variable(variable(v)),
                                          tx: PatternNonValuePlace::Placeholder,
                                      })],
                              }));
    }

    #[test]
    fn test_find_sp_variable() {
        let sym = edn::PlainSymbol::new("?x");
        let input = edn::Value::Vector(vec![edn::Value::PlainSymbol(sym.clone())]);
        assert_parses_to!(|| vector().of_exactly(Query::variable()), input, variable(sym));
    }

    #[test]
    fn test_find_scalar() {
        let sym = edn::PlainSymbol::new("?x");
        let period = edn::PlainSymbol::new(".");
        let input = edn::Value::Vector(vec![edn::Value::PlainSymbol(sym.clone()), edn::Value::PlainSymbol(period.clone())]);
        assert_parses_to!(|| vector().of_exactly(Find::find_scalar()),
                          input,
                          FindSpec::FindScalar(Element::Variable(variable(sym))));
    }

    #[test]
    fn test_find_coll() {
        let sym = edn::PlainSymbol::new("?x");
        let period = edn::PlainSymbol::new("...");
        let input = edn::Value::Vector(vec![edn::Value::PlainSymbol(sym.clone()),
                                            edn::Value::PlainSymbol(period.clone())]);
        assert_parses_to!(Find::find_coll,
                          input,
                          FindSpec::FindColl(Element::Variable(variable(sym))));
    }

    #[test]
    fn test_find_rel() {
        let vx = edn::PlainSymbol::new("?x");
        let vy = edn::PlainSymbol::new("?y");
        let input = edn::Value::Vector(vec![edn::Value::PlainSymbol(vx.clone()), edn::Value::PlainSymbol(vy.clone())]);
        assert_parses_to!(|| vector().of_exactly(Find::find_rel()),
                          input,
                          FindSpec::FindRel(vec![Element::Variable(variable(vx)),
                                                 Element::Variable(variable(vy))]));
    }

    #[test]
    fn test_find_tuple() {
        let vx = edn::PlainSymbol::new("?x");
        let vy = edn::PlainSymbol::new("?y");
        let input = edn::Value::Vector(vec![edn::Value::PlainSymbol(vx.clone()),
                                             edn::Value::PlainSymbol(vy.clone())]);
        assert_parses_to!(Find::find_tuple,
                          input,
                          FindSpec::FindTuple(vec![Element::Variable(variable(vx)),
                                                   Element::Variable(variable(vy))]));
    }

    #[test]
    fn test_natural_numbers() {
        let text = edn::Value::Text("foo".to_string());
        let neg = edn::Value::Integer(-10);
        let zero = edn::Value::Integer(0);
        let pos = edn::Value::Integer(5);

        // This is terrible, but destructuring errors is frustrating.
        let input = text.with_spans();
        let mut par = Query::natural_number();
        let x = par.parse(input.atom_stream()).err().expect("an error").errors;
        let result = format!("{:?}", x);
        assert_eq!(result, "[Other(Error(InvalidLimit(Text(\"foo\")), State { next_error: None, backtrace: None })), Expected(Borrowed(\"natural_number\"))]");

        let input = neg.with_spans();
        let mut par = Query::natural_number();
        let x = par.parse(input.atom_stream()).err().expect("an error").errors;
        let result = format!("{:?}", x);
        assert_eq!(result, "[Other(Error(InvalidLimit(Integer(-10)), State { next_error: None, backtrace: None })), Expected(Borrowed(\"natural_number\"))]");

        let input = zero.with_spans();
        let mut par = Query::natural_number();
        let x = par.parse(input.atom_stream()).err().expect("an error").errors;
        let result = format!("{:?}", x);
        assert_eq!(result, "[Other(Error(InvalidLimit(Integer(0)), State { next_error: None, backtrace: None })), Expected(Borrowed(\"natural_number\"))]");

        let input = pos.with_spans();
        let mut par = Query::natural_number();
        assert_eq!(None, par.parse(input.atom_stream()).err());
    }

    #[test]
    fn test_fn_arg_collections() {
        let vx = edn::PlainSymbol::new("?x");
        let vy = edn::PlainSymbol::new("?y");
        let input = edn::Value::Vector(vec![edn::Value::Vector(vec![edn::Value::PlainSymbol(vx.clone()),
                                            edn::Value::PlainSymbol(vy.clone())])]);

        assert_parses_to!(|| vector().of_exactly(Query::fn_arg()),
                          input,
                          FnArg::Vector(vec![FnArg::Variable(variable(vx)),
                                             FnArg::Variable(variable(vy)),
                          ]));
    }
}

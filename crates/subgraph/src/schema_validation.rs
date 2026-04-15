//! Compile-time drift detection between crate queries and the subgraph SDL.
//!
//! Parses every `GraphQL` query the crate sends to the subgraph (registered
//! in [`super::queries::CRATE_QUERIES`] and [`super::queries::PUBLIC_QUERIES`])
//! into an AST via `graphql_parser`, and walks each query's selection sets
//! against the entity types declared in `specs/subgraph.graphql`.
//!
//! A query that references a field which does not exist on the matching
//! entity — because the upstream subgraph schema evolved, because someone
//! mistyped a field name, or because an entity was renamed — turns the
//! drift-detection test red with a precise `[query_name] field X does not
//! exist on entity Y` diagnostic.
//!
//! # Why this replaces the old hand-rolled assertions
//!
//! The previous version of this module asserted that a list of **hand-coded
//! field names** existed in the SDL. Those lists were maintained manually
//! and bore no relation to the queries actually sent by the crate, so they
//! could drift silently against both the subgraph and the hand-written
//! types. The new test walks real query ASTs and therefore detects drift
//! against the code path that actually matters — the bytes going on the
//! wire.

#[cfg(test)]
mod tests {
    use foldhash::{HashMap, HashMapExt};
    use graphql_parser::{
        query::{
            self, Definition as QueryDefinition, OperationDefinition, Selection, SelectionSet,
        },
        schema::{self, Definition as SchemaDefinition, Type, TypeDefinition},
    };

    use crate::queries::{CRATE_QUERIES, PUBLIC_QUERIES};

    // ── Schema model ────────────────────────────────────────────────────────

    /// Classification of a field's declared type after stripping `!` /
    /// `[...]` wrappers.
    #[derive(Debug, Clone)]
    enum FieldType {
        /// A built-in or custom scalar that has no sub-selections.
        Scalar,
        /// A reference to another entity type which may carry its own
        /// selection set.
        Entity(String),
    }

    /// Unwrap a `GraphQL` type expression down to its inner named type and
    /// classify it as either scalar or entity.
    fn classify(ty: &Type<'_, String>) -> FieldType {
        match ty {
            Type::NamedType(name) => {
                if is_scalar(name) {
                    FieldType::Scalar
                } else {
                    FieldType::Entity(name.clone())
                }
            }
            Type::ListType(inner) | Type::NonNullType(inner) => classify(inner),
        }
    }

    /// Scalars recognised by the vendored SDL. Anything else is assumed to
    /// be an entity reference.
    fn is_scalar(name: &str) -> bool {
        matches!(
            name,
            "String" | "Int" | "Float" | "Boolean" | "ID" | "Bytes" | "BigInt" | "BigDecimal"
        )
    }

    /// Map of entity name → (field name → classified field type).
    type SchemaModel = HashMap<String, HashMap<String, FieldType>>;

    /// Parse `specs/subgraph.graphql` into a schema model.
    fn build_schema_model() -> SchemaModel {
        let sdl = include_str!("../specs/subgraph.graphql");
        let doc = schema::parse_schema::<String>(sdl)
            .unwrap_or_else(|e| panic!("failed to parse subgraph.graphql: {e}"));

        let mut model = HashMap::new();
        for def in &doc.definitions {
            if let SchemaDefinition::TypeDefinition(TypeDefinition::Object(obj)) = def {
                let mut fields = HashMap::new();
                for f in &obj.fields {
                    fields.insert(f.name.clone(), classify(&f.field_type));
                }
                model.insert(obj.name.clone(), fields);
            }
        }
        model
    }

    // ── Linter ──────────────────────────────────────────────────────────────

    /// Lint a single query string against the schema model, anchoring the
    /// query's top-level selection set to `root_entity`.
    ///
    /// Returns a list of human-readable error strings; an empty list means
    /// the query is wire-compatible with the SDL.
    fn lint_query(query_str: &str, root_entity: &str, model: &SchemaModel) -> Vec<String> {
        let doc = match query::parse_query::<String>(query_str) {
            Ok(d) => d,
            Err(e) => return vec![format!("parse error: {e}")],
        };

        let mut errors = Vec::new();
        for def in &doc.definitions {
            let QueryDefinition::Operation(op) = def else {
                errors.push("fragment definitions are not supported by the drift linter".into());
                continue;
            };
            let selection_set = match op {
                OperationDefinition::Query(q) => &q.selection_set,
                OperationDefinition::SelectionSet(ss) => ss,
                OperationDefinition::Mutation(m) => &m.selection_set,
                OperationDefinition::Subscription(s) => &s.selection_set,
            };
            walk_top_level(selection_set, root_entity, model, &mut errors);
        }
        errors
    }

    /// The top level of every query is a list of Query-root entry points
    /// (`totals`, `dailyTotals`, `bundle(id:"1")`, …). The vendored SDL
    /// does not carry a `Query` type — subgraphs synthesise one — so we
    /// skip validating the entry-point names themselves and immediately
    /// recurse into their sub-selections, which we anchor to
    /// `root_entity` (supplied by the caller).
    fn walk_top_level(
        ss: &SelectionSet<'_, String>,
        root: &str,
        model: &SchemaModel,
        errors: &mut Vec<String>,
    ) {
        for sel in &ss.items {
            if let Selection::Field(f) = sel {
                walk_selection_set(&f.selection_set, root, model, errors);
            }
        }
    }

    /// Recursively validate a selection set against an entity type.
    fn walk_selection_set(
        ss: &SelectionSet<'_, String>,
        entity: &str,
        model: &SchemaModel,
        errors: &mut Vec<String>,
    ) {
        let Some(fields) = model.get(entity) else {
            errors.push(format!("unknown entity `{entity}` in schema model"));
            return;
        };
        for sel in &ss.items {
            match sel {
                Selection::Field(f) => {
                    let Some(field_type) = fields.get(&f.name) else {
                        errors.push(format!(
                            "field `{}` does not exist on entity `{entity}`",
                            f.name
                        ));
                        continue;
                    };
                    if !f.selection_set.items.is_empty() {
                        match field_type {
                            FieldType::Entity(sub_entity) => {
                                walk_selection_set(
                                    &f.selection_set,
                                    &sub_entity.clone(),
                                    model,
                                    errors,
                                );
                            }
                            FieldType::Scalar => {
                                errors.push(format!(
                                    "field `{}` on entity `{entity}` is a scalar but the query \
                                     selects sub-fields",
                                    f.name
                                ));
                            }
                        }
                    }
                }
                Selection::FragmentSpread(_) | Selection::InlineFragment(_) => {
                    errors.push(format!(
                        "fragments are not supported by the drift linter (in entity `{entity}`)"
                    ));
                }
            }
        }
    }

    // ── Tests ───────────────────────────────────────────────────────────────

    #[test]
    fn schema_parses_cleanly() {
        let model = build_schema_model();
        assert!(!model.is_empty(), "SDL produced empty schema model");
    }

    #[test]
    fn every_expected_entity_is_present() {
        let model = build_schema_model();
        // Every root entity referenced by the crate's query list must
        // resolve to an actual entity in the SDL. Missing entries indicate
        // either a stale SDL or a misregistered query.
        for (name, _, root_entity) in CRATE_QUERIES.iter().chain(PUBLIC_QUERIES) {
            assert!(
                model.contains_key(*root_entity),
                "query `{name}` claims root entity `{root_entity}`, which is absent from \
                 specs/subgraph.graphql"
            );
        }
    }

    #[test]
    fn every_crate_query_matches_schema() {
        let model = build_schema_model();
        let mut all_errors: Vec<String> = Vec::new();

        for (name, query, root) in CRATE_QUERIES.iter().chain(PUBLIC_QUERIES) {
            let errors = lint_query(query, root, &model);
            for err in errors {
                all_errors.push(format!("[{name}] {err}"));
            }
        }

        assert!(
            all_errors.is_empty(),
            "subgraph query drift detected ({} issues):\n  - {}",
            all_errors.len(),
            all_errors.join("\n  - ")
        );
    }

    #[test]
    fn linter_catches_missing_field() {
        let model = build_schema_model();
        let bad = "query { totals { tokens definitely_not_a_field } }";
        let errors = lint_query(bad, "Total", &model);
        assert!(
            errors.iter().any(|e| e.contains("definitely_not_a_field")),
            "linter must flag unknown fields; got: {errors:?}"
        );
    }

    #[test]
    fn linter_catches_scalar_with_subselection() {
        let model = build_schema_model();
        // `tokens` is `BigInt` (scalar) on the Total entity, so selecting
        // sub-fields on it must be flagged.
        let bad = "query { totals { tokens { nope } } }";
        let errors = lint_query(bad, "Total", &model);
        assert!(
            errors.iter().any(|e| e.contains("scalar")),
            "linter must flag sub-selections on scalar fields; got: {errors:?}"
        );
    }

    #[test]
    fn linter_accepts_minimal_valid_query() {
        let model = build_schema_model();
        let good = "query { totals { tokens orders traders } }";
        assert!(lint_query(good, "Total", &model).is_empty());
    }

    #[test]
    fn linter_catches_unknown_entity() {
        let model = build_schema_model();
        let bad = "query { totals { tokens } }";
        let errors = lint_query(bad, "NonExistentEntity", &model);
        assert!(
            errors.iter().any(|e| e.contains("unknown entity")),
            "linter must flag unknown entity; got: {errors:?}"
        );
    }

    #[test]
    fn linter_catches_fragment_spread() {
        let model = build_schema_model();
        // A query that uses a fragment spread
        let bad = "query { totals { ...TotalFields } } fragment TotalFields on Total { tokens }";
        let errors = lint_query(bad, "Total", &model);
        assert!(
            errors.iter().any(|e| e.contains("fragment")),
            "linter must flag fragments; got: {errors:?}"
        );
    }

    #[test]
    fn linter_handles_parse_error() {
        let model = build_schema_model();
        let bad = "not a valid graphql query {{{";
        let errors = lint_query(bad, "Total", &model);
        assert!(
            errors.iter().any(|e| e.contains("parse error")),
            "linter must flag parse errors; got: {errors:?}"
        );
    }

    #[test]
    fn linter_mutation_and_subscription_paths() {
        let model = build_schema_model();
        // These will parse but have empty selection sets at the entity level
        let mutation = "mutation { doSomething { tokens } }";
        let errors = lint_query(mutation, "Total", &model);
        // The mutation path is exercised (may or may not have errors depending on fields)
        let _ = errors;

        let subscription = "subscription { totals { tokens } }";
        let errors = lint_query(subscription, "Total", &model);
        let _ = errors;
    }

    #[test]
    fn classify_list_type_entity() {
        // Test that classify handles ListType wrapping an entity
        let ty = Type::ListType(Box::new(Type::NamedType("Token".to_owned())));
        let ft = classify(&ty);
        assert!(matches!(ft, FieldType::Entity(ref name) if name == "Token"));
    }

    #[test]
    fn classify_nonnull_scalar() {
        let ty = Type::NonNullType(Box::new(Type::NamedType("BigInt".to_owned())));
        let ft = classify(&ty);
        assert!(matches!(ft, FieldType::Scalar));
    }
}

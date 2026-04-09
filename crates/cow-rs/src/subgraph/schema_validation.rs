//! Compile-time validation of subgraph Rust types against the `GraphQL` schema.
//!
//! Parses `specs/subgraph.graphql` and checks that every Rust response type
//! has fields matching the schema. This catches drift when the upstream
//! subgraph schema changes.
//!
//! Update the schema with `make fetch-subgraph-schema`, then run
//! `cargo test` — any field mismatch will fail loudly.

#[cfg(test)]
mod tests {
    use foldhash::{HashMap, HashMapExt, HashSet};
    use graphql_parser::schema::{Definition, TypeDefinition, parse_schema};

    /// Parse the `GraphQL` schema and build a map of type name → set of field names.
    fn schema_types() -> HashMap<String, HashSet<String>> {
        let sdl = include_str!("../../../../specs/subgraph.graphql");
        let doc = parse_schema::<String>(sdl)
            .unwrap_or_else(|e| panic!("failed to parse subgraph.graphql: {e}"));

        let mut types = HashMap::new();
        for def in &doc.definitions {
            if let Definition::TypeDefinition(TypeDefinition::Object(obj)) = def {
                let fields: HashSet<String> = obj.fields.iter().map(|f| f.name.clone()).collect();
                types.insert(obj.name.clone(), fields);
            }
        }
        types
    }

    /// Assert that a `GraphQL` type exists and contains all the given fields.
    fn assert_fields(
        types: &HashMap<String, HashSet<String>>,
        gql_type: &str,
        expected_fields: &[&str],
    ) {
        let fields = types
            .get(gql_type)
            .unwrap_or_else(|| panic!("GraphQL type `{gql_type}` not found in schema"));
        for &field in expected_fields {
            assert!(
                fields.contains(field),
                "GraphQL type `{gql_type}` is missing field `{field}` — \
                 schema has: {fields:?}"
            );
        }
    }

    // ── Type-by-type validation ──────────────────────────────────────────────

    #[test]
    fn totals_fields_match_schema() {
        let types = schema_types();
        // The Rust `Totals` type queries from the `Total` GraphQL entity.
        assert_fields(
            &types,
            "Total",
            &[
                "tokens",
                "orders",
                "traders",
                "settlements",
                "volumeUsd",
                "volumeEth",
                "feesUsd",
                "feesEth",
            ],
        );
    }

    #[test]
    fn daily_total_fields_match_schema() {
        let types = schema_types();
        assert_fields(
            &types,
            "DailyTotal",
            &[
                "timestamp",
                "orders",
                "traders",
                "tokens",
                "settlements",
                "volumeEth",
                "volumeUsd",
                "feesEth",
                "feesUsd",
            ],
        );
    }

    #[test]
    fn hourly_total_fields_match_schema() {
        let types = schema_types();
        assert_fields(
            &types,
            "HourlyTotal",
            &[
                "timestamp",
                "orders",
                "traders",
                "tokens",
                "settlements",
                "volumeEth",
                "volumeUsd",
                "feesEth",
                "feesUsd",
            ],
        );
    }

    #[test]
    fn token_fields_match_schema() {
        let types = schema_types();
        assert_fields(
            &types,
            "Token",
            &[
                "id",
                "address",
                "firstTradeTimestamp",
                "name",
                "symbol",
                "decimals",
                "totalVolume",
                "priceEth",
                "priceUsd",
                "numberOfTrades",
            ],
        );
    }

    #[test]
    fn token_daily_total_fields_match_schema() {
        let types = schema_types();
        assert_fields(
            &types,
            "TokenDailyTotal",
            &[
                "id",
                "token",
                "timestamp",
                "totalVolume",
                "totalVolumeUsd",
                "totalTrades",
                "openPrice",
                "closePrice",
                "higherPrice",
                "lowerPrice",
                "averagePrice",
            ],
        );
    }

    #[test]
    fn token_hourly_total_fields_match_schema() {
        let types = schema_types();
        assert_fields(
            &types,
            "TokenHourlyTotal",
            &[
                "id",
                "token",
                "timestamp",
                "totalVolume",
                "totalVolumeUsd",
                "totalTrades",
                "openPrice",
                "closePrice",
                "higherPrice",
                "lowerPrice",
                "averagePrice",
            ],
        );
    }

    #[test]
    fn token_trading_event_fields_match_schema() {
        let types = schema_types();
        assert_fields(&types, "TokenTradingEvent", &["id", "token", "priceUsd", "timestamp"]);
    }

    #[test]
    fn user_fields_match_schema() {
        let types = schema_types();
        assert_fields(
            &types,
            "User",
            &[
                "id",
                "address",
                "firstTradeTimestamp",
                "numberOfTrades",
                "solvedAmountEth",
                "solvedAmountUsd",
            ],
        );
    }

    #[test]
    fn settlement_fields_match_schema() {
        let types = schema_types();
        assert_fields(
            &types,
            "Settlement",
            &["id", "txHash", "firstTradeTimestamp", "solver", "txCost", "txFeeInEth"],
        );
    }

    #[test]
    fn trade_fields_match_schema() {
        let types = schema_types();
        assert_fields(
            &types,
            "Trade",
            &[
                "id",
                "timestamp",
                "gasPrice",
                "feeAmount",
                "txHash",
                "settlement",
                "buyAmount",
                "sellAmount",
                "sellAmountBeforeFees",
                "buyToken",
                "sellToken",
                "owner",
                "order",
            ],
        );
    }

    #[test]
    fn order_fields_match_schema() {
        let types = schema_types();
        assert_fields(
            &types,
            "Order",
            &[
                "id",
                "owner",
                "sellToken",
                "buyToken",
                "sellAmount",
                "buyAmount",
                "validTo",
                "appData",
                "feeAmount",
                "kind",
                "partiallyFillable",
                "status",
                "executedSellAmount",
                "executedSellAmountBeforeFees",
                "executedBuyAmount",
                "executedFeeAmount",
                "timestamp",
                "txHash",
                "isSignerSafe",
                "signingScheme",
                "uid",
            ],
        );
    }

    #[test]
    fn pair_fields_match_schema() {
        let types = schema_types();
        assert_fields(
            &types,
            "Pair",
            &["id", "token0", "token1", "volumeToken0", "volumeToken1", "numberOfTrades"],
        );
    }

    #[test]
    fn pair_daily_fields_match_schema() {
        let types = schema_types();
        assert_fields(
            &types,
            "PairDaily",
            &[
                "id",
                "token0",
                "token1",
                "timestamp",
                "volumeToken0",
                "volumeToken1",
                "numberOfTrades",
            ],
        );
    }

    #[test]
    fn pair_hourly_fields_match_schema() {
        let types = schema_types();
        assert_fields(
            &types,
            "PairHourly",
            &[
                "id",
                "token0",
                "token1",
                "timestamp",
                "volumeToken0",
                "volumeToken1",
                "numberOfTrades",
            ],
        );
    }

    #[test]
    fn bundle_fields_match_schema() {
        let types = schema_types();
        assert_fields(&types, "Bundle", &["id", "ethPriceUSD"]);
    }

    #[test]
    fn uniswap_token_fields_match_schema() {
        let types = schema_types();
        assert_fields(&types, "UniswapToken", &["id", "address", "name", "symbol", "decimals"]);
    }

    #[test]
    fn uniswap_pool_fields_match_schema() {
        let types = schema_types();
        assert_fields(
            &types,
            "UniswapPool",
            &[
                "id",
                "liquidity",
                "token0",
                "token0Price",
                "token1",
                "token1Price",
                "totalValueLockedToken0",
                "totalValueLockedToken1",
            ],
        );
    }

    // ── Schema completeness check ────────────────────────────────────────────

    #[test]
    fn all_schema_types_have_validation_tests() {
        let types = schema_types();
        let expected_types = [
            "Total",
            "DailyTotal",
            "HourlyTotal",
            "Token",
            "TokenDailyTotal",
            "TokenHourlyTotal",
            "TokenTradingEvent",
            "User",
            "Settlement",
            "Trade",
            "Order",
            "Pair",
            "PairDaily",
            "PairHourly",
            "Bundle",
            "UniswapToken",
            "UniswapPool",
        ];

        for ty in &expected_types {
            assert!(types.contains_key(*ty), "Expected GraphQL type `{ty}` not found in schema");
        }

        // Warn if schema has types we don't cover.
        for schema_type in types.keys() {
            assert!(
                expected_types.contains(&schema_type.as_str()),
                "GraphQL type `{schema_type}` exists in schema but has no \
                 validation test — add one or update the expected list"
            );
        }
    }
}

// @generated automatically by Diesel CLI.

diesel::table! {
    general_lookup (reset_id, key) {
        reset_id -> Text,
        key -> Text,
        value -> Json,
        inserted_at -> Timestamptz,
    }
}

diesel::table! {
    market_trades (id, timestamp) {
        id -> Int8,
        timestamp -> Timestamptz,
        market_symbol -> Text,
        symbol -> Text,
        trade_volume -> Int4,
        #[sql_name = "type"]
        type_ -> Text,
        supply -> Text,
        activity -> Nullable<Text>,
        purchase_price -> Int4,
        sell_price -> Int4,
    }
}

diesel::table! {
    market_transactions (market_symbol, timestamp) {
        timestamp -> Timestamptz,
        market_symbol -> Text,
        symbol -> Text,
        ship_symbol -> Text,
        #[sql_name = "type"]
        type_ -> Text,
        units -> Int4,
        price_per_unit -> Int4,
        total_price -> Int4,
    }
}

diesel::table! {
    surveys (reset_id, uuid) {
        reset_id -> Text,
        uuid -> Uuid,
        survey -> Json,
        asteroid_symbol -> Text,
        inserted_at -> Timestamptz,
        expires_at -> Timestamptz,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    general_lookup,
    market_trades,
    market_transactions,
    surveys,
);

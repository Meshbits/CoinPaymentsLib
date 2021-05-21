table! {
    accounts (id) {
        id -> Int4,
        address -> Varchar,
        viewing_key_id -> Nullable<Int4>,
        diversifier_index -> Nullable<Int4>,
        user_id -> Nullable<Int4>,
    }
}

table! {
    blocks (id) {
        id -> Int4,
        height -> Int4,
        anchor -> Bytea,
        hash -> Bytea,
        prevhash -> Bytea,
    }
}

table! {
    transactions (id) {
        id -> Int4,
        txid -> Bytea,
        block_id -> Int4,
    }
}

table! {
    viewing_keys (id) {
        id -> Int4,
        key -> Varchar,
    }
}

joinable!(accounts -> viewing_keys (viewing_key_id));
joinable!(transactions -> blocks (block_id));

allow_tables_to_appear_in_same_query!(
    accounts,
    blocks,
    transactions,
    viewing_keys,
);

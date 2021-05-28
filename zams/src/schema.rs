table! {
    accounts (id) {
        id -> Int4,
        address -> Varchar,
        viewing_key_id -> Nullable<Int4>,
        diversifier_index_high -> Nullable<Int8>,
        diversifier_index_low -> Nullable<Int8>,
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
    notes (id) {
        id -> Int4,
        tx_id -> Int4,
        vout_index -> Int4,
        value -> Int8,
        address -> Varchar,
        shielded -> Bool,
        locked -> Bool,
        spent -> Bool,
    }
}

table! {
    sapling_notes (id) {
        id -> Int4,
        diversifier -> Bytea,
        rcm -> Bytea,
        nf -> Bytea,
    }
}

table! {
    transactions (id) {
        id -> Int4,
        txhash -> Bytea,
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
joinable!(notes -> transactions (tx_id));
joinable!(sapling_notes -> notes (id));
joinable!(transactions -> blocks (block_id));

allow_tables_to_appear_in_same_query!(
    accounts,
    blocks,
    notes,
    sapling_notes,
    transactions,
    viewing_keys,
);

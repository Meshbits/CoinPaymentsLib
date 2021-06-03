CREATE TABLE IF NOT EXISTS fvks (
    id_fvk INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    extfvk TEXT NOT NULL
);
CREATE UNIQUE INDEX fvks_fvk ON fvks(extfvk);
CREATE TABLE IF NOT EXISTS accounts (
    account INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    fvk INTEGER,
    address TEXT NOT NULL,
    FOREIGN KEY (fvk) REFERENCES fvks(id_fvk)
);
CREATE UNIQUE INDEX account_address ON accounts(address);
CREATE TABLE IF NOT EXISTS blocks (
    height INTEGER PRIMARY KEY,
    hash BYTEA NOT NULL,
    time INTEGER NOT NULL,
    sapling_tree BYTEA NOT NULL
);
CREATE TABLE IF NOT EXISTS transactions (
    id_tx INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    txid BYTEA NOT NULL UNIQUE,
    created TEXT,
    block INTEGER,
    tx_index INTEGER,
    expiry_height INTEGER,
    raw BYTEA,
    FOREIGN KEY (block) REFERENCES blocks(height)
);
CREATE TABLE IF NOT EXISTS received_notes (
    id_note INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    tx INTEGER NOT NULL,
    output_index INTEGER NOT NULL,
    account INTEGER NOT NULL,
    diversifier BYTEA NOT NULL,
    address TEXT NOT NULL,
    value BIGINT NOT NULL,
    rcm BYTEA NOT NULL,
    nf BYTEA NOT NULL UNIQUE,
    is_change BOOL,
    memo BYTEA,
    spent INTEGER,
    FOREIGN KEY (tx) REFERENCES transactions(id_tx),
    FOREIGN KEY (account) REFERENCES accounts(account),
    FOREIGN KEY (spent) REFERENCES transactions(id_tx),
    CONSTRAINT tx_received_output UNIQUE (tx, output_index)
);
CREATE INDEX received_notes_address ON received_notes(address);
CREATE TABLE IF NOT EXISTS sapling_witnesses (
    id_witness INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    note INTEGER NOT NULL,
    block INTEGER NOT NULL,
    witness BYTEA NOT NULL,
    FOREIGN KEY (note) REFERENCES received_notes(id_note),
    FOREIGN KEY (block) REFERENCES blocks(height),
    CONSTRAINT witness_height UNIQUE (note, block)
);
CREATE TABLE IF NOT EXISTS sent_notes (
    id_note INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    tx INTEGER NOT NULL,
    output_index INTEGER NOT NULL,
    from_account INTEGER NOT NULL,
    address TEXT NOT NULL,
    value BIGINT NOT NULL,
    memo BYTEA,
    FOREIGN KEY (tx) REFERENCES transactions(id_tx),
    FOREIGN KEY (from_account) REFERENCES accounts(account),
    CONSTRAINT tx_send_output UNIQUE (tx, output_index)
);
-- CREATE TABLE IF NOT EXISTS chaintip (
--     id INTEGER NOT NULL PRIMARY KEY,
--     height INTEGER NOT NULL,
--     FOREIGN KEY (height) REFERENCES blocks(height)
-- );
CREATE TABLE IF NOT EXISTS utxos (
    id_utxo INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    tx_hash BYTEA NOT NULL,
    address TEXT NOT NULL,
    output_index INTEGER NOT NULL,
    value BIGINT NOT NULL,
    script BYTEA NOT NULL,
    height INTEGER NOT NULL,
    spent BOOL NOT NULL,
    spent_height INTEGER
);
CREATE INDEX utxo_tx ON utxos(tx_hash);
CREATE UNIQUE INDEX utxo_tx_idx ON utxos(tx_hash, output_index);


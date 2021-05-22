CREATE TABLE blocks (
    id INT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    height INT NOT NULL,
    anchor BYTEA NOT NULL,
    hash BYTEA NOT NULL,
    prevhash BYTEA NOT NULL
);

CREATE UNIQUE INDEX idx_block_hash ON blocks (
    hash
);

CREATE TABLE transactions (
    id INT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    txhash BYTEA NOT NULL,
    block_id INT NOT NULL,
    CONSTRAINT fk_block FOREIGN KEY(block_id) REFERENCES blocks(id)
);

CREATE UNIQUE INDEX idx_transaction_hash ON transactions (
    txhash
);

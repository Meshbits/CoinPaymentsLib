CREATE TABLE notes (
    id INT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    tx_id INT NOT NULL,
    vout_index INT NOT NULL,
    value BIGINT NOT NULL,
    address VARCHAR(120) NOT NULL,
    shielded BOOL NOT NULL,
    locked BOOL NOT NULL,
    spent BOOL NOT NULL,
    CONSTRAINT fk_tx FOREIGN KEY (tx_id) REFERENCES transactions(id)
);

CREATE UNIQUE INDEX idx_tx ON notes(
    tx_id, vout_index
);

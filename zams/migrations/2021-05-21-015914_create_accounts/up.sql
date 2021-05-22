CREATE TABLE viewing_keys (
    id INT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    key VARCHAR(400) NOT NULL
);

CREATE UNIQUE INDEX idx_viewing_key ON viewing_keys (
    key
);

CREATE TABLE accounts (
    id INT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    address VARCHAR(120) NOT NULL,
    viewing_key_id INT,
    diversifier_index_high BIGINT,
    diversifier_index_low BIGINT,
    user_id INT,
    CONSTRAINT fk_viewing_key FOREIGN KEY (viewing_key_id) REFERENCES viewing_keys(id)
);

CREATE UNIQUE INDEX idx_account_address ON accounts (
    address
);

CREATE INDEX idx_account_user ON accounts (
    user_id
);

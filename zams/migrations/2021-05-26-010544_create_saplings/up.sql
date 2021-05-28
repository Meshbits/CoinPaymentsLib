CREATE TABLE sapling_notes (
   id INT PRIMARY KEY,
   diversifier BYTEA NOT NULL,
   rcm BYTEA NOT NULL,
   nf BYTEA NOT NULL,
   CONSTRAINT fk_notes FOREIGN KEY (id) REFERENCES notes(id)
);

CREATE TABLE attributions (
  id SERIAL PRIMARY KEY,
  name VARCHAR UNIQUE NOT NULL
);

ALTER TABLE photos ADD COLUMN attribution_id
INTEGER REFERENCES attributions (id);

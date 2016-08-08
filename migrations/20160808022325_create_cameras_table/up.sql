CREATE TABLE cameras (
  id           SERIAL PRIMARY KEY,
  manufacturer VARCHAR NOT NULL,
  model        VARCHAR NOT NULL
);

CREATE UNIQUE INDEX cameras_idx ON cameras (manufacturer, model);

ALTER TABLE photos ADD COLUMN camera_id INTEGER REFERENCES cameras (id);

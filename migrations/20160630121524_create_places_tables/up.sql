CREATE TABLE places (
  id SERIAL PRIMARY KEY,
  slug VARCHAR UNIQUE NOT NULL,
  place_name VARCHAR UNIQUE NOT NULL
);

CREATE UNIQUE INDEX places_slug_idx ON places (slug);
CREATE UNIQUE INDEX places_name_idx ON places (place_name);

CREATE TABLE photo_places (
  id SERIAL PRIMARY KEY,
  photo_id INTEGER NOT NULL REFERENCES photos (id),
  place_id INTEGER NOT NULL REFERENCES places (id)
);

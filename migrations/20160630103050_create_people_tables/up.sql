CREATE TABLE people (
  id SERIAL PRIMARY KEY,
  slug VARCHAR UNIQUE NOT NULL,
  person_name VARCHAR UNIQUE NOT NULL
);

CREATE UNIQUE INDEX people_slug_idx ON people (slug);
CREATE UNIQUE INDEX people_name_idx ON people (person_name);

CREATE TABLE photo_people (
  id SERIAL PRIMARY KEY,
  photo_id INTEGER NOT NULL REFERENCES photos (id),
  person_id INTEGER NOT NULL REFERENCES people (id)
);

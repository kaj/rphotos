CREATE TABLE tags (
  id SERIAL PRIMARY KEY,
  slug VARCHAR UNIQUE NOT NULL,
  tag_name VARCHAR UNIQUE NOT NULL
);

CREATE UNIQUE INDEX tags_slug_idx ON tags (slug);
CREATE UNIQUE INDEX tags_name_idx ON tags (tag_name);

CREATE TABLE photo_tags (
  id SERIAL PRIMARY KEY,
  photo_id INTEGER NOT NULL REFERENCES photos (id),
  tag_id INTEGER NOT NULL REFERENCES tags (id)
);

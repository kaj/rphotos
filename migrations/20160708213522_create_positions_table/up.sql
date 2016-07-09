-- Rather than using floating points or DECIMAL(8,5) or something like
-- that, lat and long are stored as signed microdegrees integer values.
CREATE TABLE positions (
  id SERIAL PRIMARY KEY,
  photo_id INTEGER NOT NULL REFERENCES photos (id),
  latitude INTEGER NOT NULL,
  longitude INTEGER NOT NULL
);

CREATE INDEX positions_photo_idx ON positions (photo_id);
CREATE INDEX positions_lat_idx ON positions (latitude);
CREATE INDEX positions_long_idx ON positions (longitude);

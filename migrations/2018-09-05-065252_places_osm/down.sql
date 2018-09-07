ALTER TABLE places DROP COLUMN osm_id;
ALTER TABLE places DROP COLUMN osm_level;

CREATE UNIQUE INDEX places_name_idx ON places (place_name);

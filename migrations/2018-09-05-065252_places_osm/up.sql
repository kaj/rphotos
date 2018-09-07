-- Add fields for OpenStreetMap-based locations to places table.
ALTER TABLE places ADD COLUMN osm_id BIGINT UNIQUE;
ALTER TABLE places ADD COLUMN osm_level SMALLINT;

CREATE INDEX places_osm_idx ON places (osm_id);
CREATE INDEX places_osml_idx ON places (osm_level);

DROP INDEX places_name_idx;
CREATE UNIQUE INDEX places_name_idx ON places (place_name, osm_level);

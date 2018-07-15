-- Each photo may only have one single position
DROP INDEX positions_photo_idx;
CREATE UNIQUE INDEX positions_photo_idx ON positions (photo_id);

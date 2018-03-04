-- Add width and height to photos
-- Intially make them nullable, to make it possible to apply the
-- migration to an existing database.  A NOT NULL constraint should
-- be added later, when all photos has sizes.
ALTER TABLE photos ADD COLUMN width INTEGER;
ALTER TABLE photos ADD COLUMN height INTEGER;

CREATE TABLE photos (
  id SERIAL PRIMARY KEY,
  path VARCHAR UNIQUE NOT NULL,
  date TIMESTAMP,
  grade SMALLINT,
  rotation SMALLINT NOT NULL
);

CREATE UNIQUE INDEX photos_path_idx ON photos (path);
CREATE INDEX photos_date_idx ON photos (date DESC NULLS LAST);
CREATE INDEX photos_grade_idx ON photos (grade DESC NULLS LAST);

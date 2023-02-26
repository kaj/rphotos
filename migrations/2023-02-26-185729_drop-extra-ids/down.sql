alter table photo_tags drop constraint photo_tags_pkey;
alter table photo_tags add column id serial primary key;
alter table photo_people drop constraint photo_people_pkey;
alter table photo_people add column id serial primary key;
alter table photo_places drop constraint photo_places_pkey;
alter table photo_places add column id serial primary key;

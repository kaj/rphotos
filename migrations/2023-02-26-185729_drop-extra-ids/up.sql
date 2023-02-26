alter table photo_tags drop column id;
alter table photo_tags add primary key (photo_id, tag_id);

alter table photo_people drop column id;
alter table photo_people add primary key (photo_id, person_id);

alter table photo_places drop column id;
alter table photo_places add primary key (photo_id, place_id);

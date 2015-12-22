create table public.photo (
   id       serial primary key,
   path     varchar(100) unique not null,
   date     timestamp with time zone,
   grade    smallint,
   rotation smallint not null
);

create table public.tag (
  id   serial primary key,
  tag  varchar(100) unique not null,
  slug varchar(100) unique not null
);
create table public.photo_tag (
  photo  integer not null references public.photo (id),
  tag    integer not null references public.tag   (id)
);

create table public.person (
  id   serial primary key,
  name varchar(100) unique not null,
  slug varchar(100) unique not null
);
create table public.photo_person (
  photo  integer not null references public.photo  (id),
  person integer not null references public.person (id)
);

create table public.place (
  id    serial primary key,
  place varchar(100) unique not null,
  slug  varchar(100) unique not null
);
create table public.photo_place (
  photo  integer not null references public.photo (id),
  place  integer not null references public.place (id)
);

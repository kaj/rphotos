create table public.photo (
   id    serial primary key,
   path  varchar(100) unique not null
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

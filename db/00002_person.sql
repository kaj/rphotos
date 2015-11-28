create table public.person (
  id   serial primary key,
  name varchar(100) unique not null,
  slug varchar(100) unique not null
);

create table public.photo_person (
  photo  integer not null references public.photo  (id),
  person integer not null references public.person (id)
);

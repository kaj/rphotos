-- SQL functions for handling dates

create function year_of_timestamp(arg timestamp with time zone)
  returns smallint
  language sql immutable strict parallel safe
  as $func$ select cast(date_part('year', arg at time zone 'UTC') as smallint); $func$;

create function month_of_timestamp(arg timestamp with time zone)
  returns smallint
  language sql immutable strict parallel safe
  as $func$ select cast(date_part('month', arg at time zone 'UTC') as smallint); $func$;

create function day_of_timestamp(arg timestamp with time zone)
  returns smallint
  language sql immutable strict parallel safe
  as $func$ select cast(date_part('day', arg at time zone 'UTC') as smallint); $func$;

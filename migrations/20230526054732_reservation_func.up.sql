-- if user_id is null, find all reservations within during for the resource
-- if resource_id is null, find all reservations during the time span for the user
-- if both user_id and resource_id are null, find all reservations during the time span
-- if both set, find all reservations for the user and resource during the time span
CREATE OR REPLACE FUNCTION rsvp.query(
  uid text,
  rid text,
  _start timestamp with time zone,
  _end timestamp with time zone,
  status rsvp.reservation_status,
  is_desc boolean DEFAULT false
) RETURNS TABLE (LIKE rsvp.reservations)
AS $$
DECLARE
  _during tstzrange;
  _sql text;
BEGIN
  -- if _start or _end is null, use infinity
  _during := tstzrange(
    COALESCE(_start, '-infinity'::timestamp with time zone),
    COALESCE(_end, 'infinity'::timestamp with time zone),
    '[)'
  );

  -- format the query based on the parameters
  _sql = format(
    'SELECT * FROM rsvp.reservations WHERE %L @> timespan AND status = %L AND %s ORDER BY lower(timespan) %s ',
    _during,
    status,
    CASE
      WHEN uid IS NULL AND rid IS NULL THEN 'TRUE'
      WHEN uid IS NULL THEN 'resource_id = ' || quote_literal(rid)
      WHEN rid IS NULL THEN 'user_id = ' || quote_literal(uid)
      ELSE 'user_id = ' || quote_literal(uid) || ' AND resource_id = ' || quote_literal(rid)
    END,
    CASE
      WHEN is_desc THEN 'DESC'
      ELSE 'ASC'
    END
  );

  -- log the _sql
  RAISE NOTICE '%', _sql;

  -- execute the _sql
  RETURN QUERY EXECUTE _sql;

END;
$$ LANGUAGE plpgsql;


CREATE OR REPLACE FUNCTION rsvp.filter(
  uid text,
  rid text,
  status rsvp.reservation_status,
  cursor bigint DEFAULT NULL,
  is_desc boolean DEFAULT FALSE,
  page_size integer DEFAULT 10
) RETURNS TABLE (LIKE rsvp.reservations)
AS $$
DECLARE
  _sql text;
  _offset bigint;
BEGIN
  -- set page_size to 10 if it is less than 10, or more than 100
  IF page_size < 10 OR page_size > 100 THEN
    page_size := 10;
  END IF;

  -- if cursor is NULL, set it to 0 if is_desc is false, or to the max bigint if is_desc is TRUE
  IF cursor IS NULL OR cursor < 0 THEN
    IF is_desc THEN
      cursor := 9223372036854775807;
    ELSE
      cursor := 0;
    END IF;
  END IF;

  -- format the query based on the parameters
  _sql = format(
    'SELECT * FROM rsvp.reservations WHERE %s AND status = %L AND %s ORDER BY id %s LIMIT %L::integer',
    CASE
      -- not include the cursor
      WHEN is_desc THEN 'id < ' || cursor
      ELSE 'id > ' || cursor
    END,
    status,
    CASE
      WHEN uid IS NULL AND rid IS NULL THEN 'TRUE'
      WHEN uid IS NULL THEN 'resource_id = ' || quote_literal(rid)
      WHEN rid IS NULL THEN 'user_id = ' || quote_literal(uid)
      ELSE 'user_id = ' || quote_literal(uid) || ' AND resource_id = ' || quote_literal(rid)
    END,
    CASE
      WHEN is_desc THEN 'DESC'
      ELSE 'ASC'
    END,
    page_size + 1
  );

  -- log the _sql
  RAISE NOTICE '%', _sql;

  -- execute the _sql
  RETURN QUERY EXECUTE _sql;

END;
$$ LANGUAGE plpgsql;

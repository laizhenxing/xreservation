-- if user_id is null, find all reservations within during for the resource
-- if resource_id is null, find all reservations during the time span for the user
-- if both user_id and resource_id are null, find all reservations during the time span
-- if both set, find all reservations for the user and resource during the time span
CREATE OR REPLACE FUNCTION rsvp.query(uid text, rid text, during TSTZRANGE) RETURNS TABLE (LIKE rsvp.reservations)
AS $$
BEGIN
   -- if both user_id and resource_id are null, find all reservations during the time span
   IF uid IS NULL AND rid IS NULL THEN
       RETURN QUERY SELECT * FROM rsvp.reservations WHERE timespan && during;
    -- if user_id is null, find all reservations during the time span for the resource
    ELSIF uid IS NULL THEN
         RETURN QUERY SELECT * FROM rsvp.reservations WHERE resource_id = rid AND during @> timespan;
    -- if resource_id is null, find all reservations during the time span for the user
    ELSIF rid IS NULL THEN
         RETURN QUERY SELECT * FROM rsvp.reservations WHERE user_id = uid AND during @> timespan;
    -- if both set, find all reservations for the user and resource during the time span
    ELSE
         RETURN QUERY SELECT * FROM rsvp.reservations WHERE user_id = uid AND resource_id = rid AND during @> timespan;
    END IF;
END;
$$ LANGUAGE plpgsql;

CREATE FUNCTION reject_version_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 RAISE EXCEPTION 'Published problem versions are immutable';
END;
$$;
CREATE TRIGGER versions_immutable BEFORE UPDATE OR DELETE ON versions
FOR EACH ROW EXECUTE FUNCTION reject_version_change();

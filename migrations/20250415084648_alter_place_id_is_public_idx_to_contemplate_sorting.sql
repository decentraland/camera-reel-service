CREATE INDEX CONCURRENTLY idx_place_id_is_public_created_at_desc ON images ((metadata->>'placeId'), is_public, created_at DESC);
DROP INDEX CONCURRENTLY idx_place_id_is_public;
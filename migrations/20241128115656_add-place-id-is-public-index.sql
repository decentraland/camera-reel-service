CREATE INDEX idx_place_id_is_public ON images ((metadata->>'placeId'), is_public) INCLUDE (created_at);

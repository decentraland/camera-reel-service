CREATE INDEX IF NOT EXISTS images_place_id_idx ON images ((metadata->>'placeId'));

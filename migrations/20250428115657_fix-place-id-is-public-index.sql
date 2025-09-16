-- Drop the existing index
DROP INDEX IF EXISTS idx_place_id_is_public;

-- Create a new index with the correct structure
CREATE INDEX idx_place_id_is_public ON images ((metadata->>'placeId'), is_public) INCLUDE (created_at); 
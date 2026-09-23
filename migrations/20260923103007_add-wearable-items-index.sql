-- Which collection items are worn in a photo, as `<contract>-<itemId>`: the identity the marketplace
-- uses for an item, with the per-NFT token id of the URN dropped so every copy of an item matches the
-- one item a shopper is looking at.
--
-- Immutable so it can be indexed. Rows whose metadata does not hold the arrays this walks (an older
-- shape, or a photo of an empty scene) contribute nothing rather than failing the index build.
CREATE OR REPLACE FUNCTION image_wearable_items(metadata jsonb)
RETURNS text[]
LANGUAGE sql
IMMUTABLE
PARALLEL SAFE
AS $$
    SELECT coalesce(
        array_agg(DISTINCT lower(split_part(wearable, ':', 5) || '-' || split_part(wearable, ':', 6))),
        ARRAY[]::text[]
    )
    FROM jsonb_array_elements(
        CASE
            WHEN jsonb_typeof(metadata -> 'visiblePeople') = 'array' THEN metadata -> 'visiblePeople'
            ELSE '[]'::jsonb
        END
    ) AS person,
    jsonb_array_elements_text(
        CASE
            WHEN jsonb_typeof(person -> 'wearables') = 'array' THEN person -> 'wearables'
            ELSE '[]'::jsonb
        END
    ) AS wearable
    WHERE wearable LIKE 'urn:decentraland:%:collections-v2:%';
$$;

CREATE INDEX IF NOT EXISTS idx_images_wearable_items
    ON images USING gin (image_wearable_items(metadata));

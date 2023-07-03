CREATE TABLE IF NOT EXISTS images (
    id UUID PRIMARY KEY,
    photographer TEXT NOT NULL,
    location_x INTEGER NOT NULL,
    location_y INTEGER NOT NULL,
    url TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT now()
);

CREATE TABLE IF NOT EXISTS image_tags (
    image_id UUID REFERENCES images(id) ON DELETE CASCADE,
    tag_name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS image_users (
    id UUID PRIMARY KEY,
    image_id UUID REFERENCES images(id) ON DELETE CASCADE,
    user_address TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS user_wearables (
    user_id UUID REFERENCES image_users(id) ON DELETE CASCADE,
    wearable TEXT NOT NULL
);


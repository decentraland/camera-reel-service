CREATE TABLE IF NOT EXISTS images (
    id UUID PRIMARY KEY,
    user_address TEXT NOT NULL,
    url TEXT NOT NULL,
    metadata JSONB NOT NULL,
    created_at TIMESTAMP DEFAULT now()
);


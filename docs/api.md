# Camera Reel API

POST /api/images [authenticated]: upload image file and associated metadata
DELETE /api/images/{image_id} [authenticated]: delete image file and associated metadata
GET /api/users/{user_address}/images [authenticated]: list images' metadata of current user address => Image []

GET /api/images/{image_id}: redirects to S3 image file
GET /api/images/{image_id}/metadata: image metadata => Image

Image {
    id: String,
    url: String,
    metadata: Metadata,
}

Metadata {
    photographer: String,
    tags: String[],
    users: String[],
    wearables: String[],
}

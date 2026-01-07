# AI Agent Context

**Service Purpose:** Manages camera images captured from Decentraland Explorer. Provides upload, storage, retrieval, and metadata management for user-generated screenshots with visibility controls (public/private) and place associations.

**Key Capabilities:**

- Uploads and stores images with metadata (coordinates, scene, timestamp, visibility)
- Manages image visibility settings (public/private) per user
- Associates images with places (parcels/scenes) for discovery
- Provides user galleries and place-based image collections
- Supports image deletion and metadata updates
- Generates OpenAPI documentation via utoipa crate

**Communication Pattern:** Synchronous HTTP REST API with optional authentication

**Technology Stack:**

- Runtime: Rust (compiled binary)
- Language: Rust (edition 2021)
- HTTP Framework: Actix Web 4.x
- Database: PostgreSQL (via SQLx)
- Storage: AWS S3 or MinIO (image files)
- Authentication: Signed Fetch (ADR-44) for authenticated endpoints

**External Dependencies:**

- Databases: PostgreSQL (image metadata, user associations, place mappings)
- Storage: AWS S3 or MinIO (actual image file storage)
- Authentication: Ethereum signature validation (Signed Fetch middleware)
- Events: AWS SNS (optional, for event notifications)

**Key Concepts:**

- **Image Visibility**: Images can be marked as public or private. Private images are only visible to the owner, while public images can be discovered by place associations.
- **Place Association**: Images can be associated with places (parcels/scenes) via metadata, enabling place-based discovery and galleries.
- **Metadata Structure**: Image metadata includes coordinates (x, y, z), scene information, timestamp, and place ID for rich context.
- **Signed Fetch Authentication**: Uses ADR-44 specification for Ethereum-based authentication, allowing users to prove ownership of their Ethereum address.

**API Specification:** OpenAPI docs available at `{server}/api/docs/ui` (Swagger UI) and `{server}/api/docs/openapi.json`

**Database notes:**

- Images are stored with UUID primary keys
- User addresses are stored as TEXT (Ethereum addresses)
- Metadata is stored as JSONB for flexible schema
- Indexes on `user_address`, `place_id` (from metadata), `is_public`, and composite indexes for efficient queries
- Thumbnail URLs are stored separately from full image URLs for performance optimization


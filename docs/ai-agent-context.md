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

**API Specification:** OpenAPI docs available at `{server}/api/docs/ui` (Swagger UI) and `{server}/api/docs/openapi.json`

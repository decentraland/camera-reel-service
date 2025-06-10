use serde::__private::fmt::Debug;
use sqlx::pool::PoolConnection;
use sqlx::types::{chrono, Uuid};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use sqlx::{Error as DBError, Postgres, QueryBuilder};

use std::str::FromStr;

use crate::api::{Image, Metadata};

pub type DBResult<V> = Result<V, DBError>;

#[derive(Debug, Clone)]
pub struct Database {
    pool: PgPool,
}

pub struct DatabaseOptions {
    url: String,
    pool_options: PgPoolOptions,
}

impl DatabaseOptions {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            pool_options: PgPoolOptions::new(),
        }
    }

    pub async fn connect(self) -> DBResult<Database> {
        let pg_options = PgConnectOptions::from_str(&self.url).unwrap();
        let pool = self.pool_options.connect_with(pg_options).await?;

        Ok(Database::new(pool))
    }
}

impl Database {
    fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_connection(&self) -> DBResult<PoolConnection<Postgres>> {
        self.pool.acquire().await
    }

    async fn migrate(&self) -> Result<(), sqlx::migrate::MigrateError> {
        sqlx::migrate!("./migrations").run(&self.pool).await
    }

    pub async fn from_url(url: &str) -> DBResult<Self> {
        let mut db_options = DatabaseOptions::new(url);
        db_options.pool_options = db_options
            .pool_options
            .min_connections(5)
            .max_connections(50);
        match db_options.connect().await {
            Ok(db) => {
                db.migrate().await?;
                Ok(db)
            }
            Err(e) => Err(e),
        }
    }

    pub async fn get_image(&self, id: &str) -> DBResult<DBImage> {
        let image = sqlx::query_as::<_, DBImage>("SELECT * FROM images WHERE id = $1")
            .bind(parse_uuid(id)?)
            .fetch_one(&self.pool)
            .await?;

        Ok(image)
    }

    fn build_images_query<'a>(
        &self,
        filter_field: &str,
        filter_value: &'a [String],
        public_only: bool,
        initial_clause: &'a str,
    ) -> Result<QueryBuilder<'a, Postgres>, DBError> {
        let mut query_builder = QueryBuilder::new(initial_clause);

        query_builder.push(" WHERE ");
        match filter_field {
            "user_address" => {
                query_builder.push("user_address = ");
                query_builder.push_bind(filter_value[0].to_lowercase());
            }
            "place_id" => {
                query_builder.push("metadata->>'placeId' = ");
                let uuid = parse_uuid(&filter_value[0])?.to_string();
                query_builder.push_bind(uuid);
            }
            "places_ids" => {
                query_builder.push("metadata->>'placeId' = ANY(");
                query_builder.push_bind(filter_value);
                query_builder.push(")");
            }
            _ => {
                tracing::error!("Unsupported filter field: {}", filter_field);
                return Err(DBError::Protocol(format!(
                    "Unsupported filter field: {}",
                    filter_field
                )));
            }
        }

        if public_only {
            query_builder.push(" AND is_public = true");
        }

        Ok(query_builder)
    }

    async fn get_images(
        &self,
        filter_field: &str,
        filter_value: &[String],
        offset: i64,
        limit: i64,
        public_only: bool,
    ) -> DBResult<Vec<DBImage>> {
        let mut query_builder = self.build_images_query(
            filter_field,
            filter_value,
            public_only,
            "SELECT * FROM images",
        )?;

        query_builder
            .push(" ORDER BY created_at DESC LIMIT ")
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind(offset);

        let query = query_builder.build_query_as::<DBImage>();

        let images = query.fetch_all(&self.pool).await?;

        Ok(images)
    }

    async fn get_images_count(
        &self,
        filter_field: &str,
        filter_value: &[String],
        public_only: bool,
    ) -> DBResult<u64> {
        let mut query_builder = self.build_images_query(
            filter_field,
            filter_value,
            public_only,
            "SELECT COUNT(*) FROM images",
        )?;

        let query = query_builder.build_query_scalar::<i64>();
        let count = query.fetch_one(&self.pool).await?;

        Ok(count as u64)
    }

    pub async fn get_user_images(
        &self,
        user: &str,
        offset: i64,
        limit: i64,
        public_only: bool,
    ) -> DBResult<Vec<DBImage>> {
        self.get_images(
            "user_address",
            &[user.to_string()],
            offset,
            limit,
            public_only,
        )
        .await
    }

    pub async fn get_place_images(
        &self,
        place_id: &str,
        offset: i64,
        limit: i64,
    ) -> DBResult<Vec<DBImage>> {
        self.get_images("place_id", &[place_id.to_string()], offset, limit, true)
            .await
    }

    pub async fn get_multiple_places_images(
        &self,
        places_ids: &[String],
        offset: i64,
        limit: i64,
    ) -> DBResult<Vec<DBImage>> {
        self.get_images("places_ids", places_ids, offset, limit, true)
            .await
    }

    pub async fn get_user_images_count(&self, user: &str, public_only: bool) -> DBResult<u64> {
        self.get_images_count("user_address", &[user.to_string()], public_only)
            .await
    }

    pub async fn get_place_images_count(&self, place_id: &str) -> DBResult<u64> {
        self.get_images_count("place_id", &[place_id.to_string()], true)
            .await
    }

    pub async fn get_multiple_places_images_count(&self, places_ids: &[String]) -> DBResult<u64> {
        self.get_images_count("places_ids", places_ids, true).await
    }

    pub async fn delete_image(&self, id: &str) -> DBResult<()> {
        sqlx::query("DELETE FROM images WHERE id = $1")
            .bind(parse_uuid(id)?)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn insert_image(&self, image: &Image) -> DBResult<()> {
        sqlx::query("INSERT INTO images (id, user_address, url, thumbnail_url, is_public, metadata) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(parse_uuid(&image.id)?)
            .bind(image.metadata.user_address.to_lowercase())
            .bind(&image.url)
            .bind(&image.thumbnail_url)
            .bind(image.is_public)
            .bind(sqlx::types::Json(&image.metadata))
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn update_image_visibility(&self, id: &str, is_public: &bool) -> DBResult<()> {
        sqlx::query("UPDATE images SET is_public = $1 WHERE id = $2")
            .bind(is_public)
            .bind(parse_uuid(id)?)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

fn parse_uuid(uuid: &str) -> Result<Uuid, DBError> {
    Uuid::parse_str(uuid).map_err(|_| DBError::Protocol("Invalid UUID".to_string()))
}

#[derive(sqlx::FromRow, Debug)]
pub struct DBImage {
    pub id: Uuid,
    pub user_address: String,
    pub url: String,
    pub thumbnail_url: String,
    pub is_public: bool,
    pub created_at: chrono::NaiveDateTime,
    pub metadata: sqlx::types::Json<Metadata>,
}

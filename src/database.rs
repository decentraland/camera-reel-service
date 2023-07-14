use serde::__private::fmt::Debug;
use sqlx::pool::PoolConnection;
use sqlx::types::{chrono, Uuid};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use sqlx::{Error as DBError, Postgres};

use std::str::FromStr;

use crate::{Image, Metadata};

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

    pub async fn get_user_images(
        &self,
        user: &str,
        offset: i64,
        limit: i64,
    ) -> DBResult<Vec<DBImage>> {
        let images = sqlx::query_as::<_, DBImage>("SELECT * FROM images WHERE user_address = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3")
            .bind(user.to_lowercase())
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        Ok(images)
    }

    pub async fn delete_image(&self, id: &str) -> DBResult<()> {
        sqlx::query("DELETE FROM images WHERE id = $1")
            .bind(parse_uuid(id)?)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn insert_image(&self, image: &Image) -> DBResult<()> {
        sqlx::query("INSERT INTO images (id, user_address, url, metadata) VALUES ($1, $2, $3, $4)")
            .bind(parse_uuid(&image.id)?)
            .bind(&image.metadata.user_address.to_lowercase())
            .bind(&image.url)
            .bind(sqlx::types::Json(&image.metadata))
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

fn parse_uuid(uuid: &str) -> Result<Uuid, DBError> {
    Uuid::parse_str(uuid).map_err(|_| DBError::Protocol("Invalid UUID".to_string()))
}

#[derive(sqlx::FromRow)]
pub struct DBImage {
    pub id: Uuid,
    pub user_address: String,
    pub url: String,
    pub created_at: chrono::NaiveDateTime,
    pub metadata: sqlx::types::Json<Metadata>,
}

use serde::__private::fmt::Debug;
use sqlx::types::chrono;
use sqlx::Error as DBError;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};

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
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(image)
    }

    pub async fn get_user_images(
        &self,
        user: &str,
        offset: u64,
        limit: u64,
    ) -> DBResult<Vec<DBImage>> {
        let images = sqlx::query_as::<_, DBImage>("SELECT * FROM images WHERE user_address = $1")
            .bind(user)
            .fetch_all(&self.pool)
            .await?;
        Ok(images)
    }

    pub async fn delete_image(&self, id: &str) -> DBResult<()> {
        sqlx::query("DELETE FROM images WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn insert_image(&self, image: &Image) -> DBResult<()> {
        sqlx::query(
            "INSERT INTO images (id, user_address, url, metadata) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&image.id)
        .bind(&image.metadata.user_address)
        .bind(&image.url)
        .bind(sqlx::types::Json(&image.metadata))
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[derive(sqlx::FromRow)]
pub struct DBImage {
    pub id: String,
    pub user_address: String,
    pub url: String,
    pub created_at: chrono::NaiveDateTime,
    pub metadata: sqlx::types::Json<Metadata>,
}

use serde::__private::fmt::Debug;
use sqlx::types::chrono;
use sqlx::Error as DBError;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};

use std::error::Error as StdError;
use std::str::FromStr;

use crate::{Image, Metadata};
//
// #[derive(Debug, Error)]
// pub enum DBError {
//     #[error("Unable to connect to DB")]
//     UnableToConnect(Error),
//
//     #[error("Unable to migrate: {0}")]
//     MigrationError(BoxDynError),
//
//     #[error("Row has incorrect data: {0}")]
//     RowCorrupted(BoxDynError),
//
//     #[error("Not a UUID given")]
//     NotUUID,
//
//     #[error("Not found")]
//     RowNotFound,
// }

/// Convenience type alias for grouping driver-specific errors
pub type BoxDynError = Box<dyn StdError + 'static + Send + Sync>;

/// Generic result data structure
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

    async fn get_image_metadata(&self, image: &DBImage) -> DBResult<Metadata> {
        let tags: Vec<Tag> =
            sqlx::query_as::<_, Tag>("SELECT tag_name FROM image_tags WHERE image_id = $1")
                .bind(&image.id)
                .fetch_all(&self.pool)
                .await?;

        let wearables: Vec<Wearable> = sqlx::query_as::<_, Wearable>(
            "SELECT wearable FROM image_wearables WHERE image_id = $1",
        )
        .bind(&image.id)
        .fetch_all(&self.pool)
        .await?;

        let users: Vec<User> =
            sqlx::query_as::<_, User>("SELECT user_address FROM image_user WHERE image_id = $1")
                .bind(&image.id)
                .fetch_all(&self.pool)
                .await?;

        Ok(Metadata {
            tags: tags.into_iter().map(|t| t.tag_name).collect(),
            wearables: wearables.into_iter().map(|w| w.wearable).collect(),
            users: users.into_iter().map(|u| u.user_address).collect(),
            photographer: image.photographer.to_string(),
            location: (image.location_x, image.location_y),
            timestamp: image.created_at.timestamp(),
        })
    }

    pub async fn get_image(&self, id: &str) -> DBResult<Image> {
        let image = sqlx::query_as::<_, DBImage>("SELECT * FROM images WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        let metadata = self.get_image_metadata(&image).await?;
        Ok(Image {
            id: image.id,
            url: image.url,
            metadata,
        })
    }

    pub async fn get_user_images(&self, user: &str) -> DBResult<Vec<Image>> {
        let images: Vec<DBImage> =
            sqlx::query_as::<_, DBImage>("SELECT * FROM images WHERE photographer = $1")
                .bind(user)
                .fetch_all(&self.pool)
                .await?;

        let mut result = vec![];
        for image in images {
            let metadata = self.get_image_metadata(&image).await?;
            result.push(Image {
                id: image.id,
                url: image.url,
                metadata,
            });
        }
        Ok(result)
    }

    pub async fn delete_image(&self, id: &str) -> DBResult<()> {
        sqlx::query("DELETE FROM images WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn insert_image(&self, image: &Image) -> DBResult<()> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query("INSERT INTO images (id, photographer, location_x, location_y, url) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(&image.id)
            .bind(&image.metadata.photographer)
            .bind(image.metadata.location.0)
            .bind(image.metadata.location.1)
            .bind(&image.url)
            .execute(&mut transaction)
            .await?;

        for tag in &image.metadata.tags {
            sqlx::query("INSERT INTO image_tags (image_id, tag_name) VALUES ($1, $2)")
                .bind(&image.id)
                .bind(tag)
                .execute(&mut transaction)
                .await?;
        }

        for wearable in &image.metadata.wearables {
            sqlx::query("INSERT INTO image_wearables (image_id, wearable) VALUES ($1, $2)")
                .bind(&image.id)
                .bind(wearable)
                .execute(&mut transaction)
                .await?;
        }

        for user in &image.metadata.users {
            sqlx::query("INSERT INTO image_user (image_id, user_address) VALUES ($1, $2)")
                .bind(&image.id)
                .bind(user)
                .execute(&mut transaction)
                .await?;
        }
        transaction.commit().await?;

        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct DBImage {
    id: String,
    photographer: String,
    location_x: i32,
    location_y: i32,
    url: String,
    created_at: chrono::NaiveDateTime,
}

#[derive(sqlx::FromRow)]
struct Tag {
    tag_name: String,
}

#[derive(sqlx::FromRow)]
struct Wearable {
    wearable: String,
}

#[derive(sqlx::FromRow)]
struct User {
    user_address: String,
}

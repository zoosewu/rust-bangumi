use diesel::r2d2;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Record not found")]
    NotFound,

    #[error("Database error: {0}")]
    Database(#[from] diesel::result::Error),

    #[error("Connection pool error: {0}")]
    Pool(String),

    #[error("Task join error: {0}")]
    TaskJoin(String),
}

impl From<r2d2::PoolError> for RepositoryError {
    fn from(e: r2d2::PoolError) -> Self {
        RepositoryError::Pool(e.to_string())
    }
}

impl From<tokio::task::JoinError> for RepositoryError {
    fn from(e: tokio::task::JoinError) -> Self {
        RepositoryError::TaskJoin(e.to_string())
    }
}

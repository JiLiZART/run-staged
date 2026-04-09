mod error;
pub use error::AppError;

pub async fn run() -> Result<(), AppError> {
    Ok(())
}

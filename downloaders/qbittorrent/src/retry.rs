use std::time::Duration;
use tokio::time::sleep;

/// Retries an async operation with exponential backoff strategy.
///
/// # Arguments
/// * `max_retries` - Maximum number of retry attempts
/// * `initial_delay` - Initial delay between retries
/// * `operation` - The async operation to retry
///
/// # Returns
/// * `Ok(T)` if the operation succeeds within max_retries attempts
/// * `Err(E)` if all attempts fail
///
/// # Example
/// ```ignore
/// let result = retry_with_backoff(3, Duration::from_secs(1), || async {
///     reqwest::get("https://example.com").await
/// }).await;
/// ```
pub async fn retry_with_backoff<F, Fut, T, E>(
    max_retries: u32,
    initial_delay: Duration,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut delay = initial_delay;

    for attempt in 1..=max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt == max_retries {
                    tracing::error!("Operation failed after {} attempts: {}", max_retries, e);
                    return Err(e);
                }

                tracing::warn!("Attempt {} failed: {}. Retrying in {:?}", attempt, e, delay);
                sleep(delay).await;
                delay = delay.saturating_mul(2); // Exponential backoff
            }
        }
    }

    unreachable!()
}

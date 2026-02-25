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

                tracing::warn!(
                    "Attempt {} failed: {}. Retrying in {:?}",
                    attempt,
                    e,
                    delay
                );
                sleep(delay).await;
                delay = delay.saturating_mul(2);
            }
        }
    }

    unreachable!()
}

/// 向 Core 註冊，指數退避重試直到成功
pub async fn register_with_core_backoff(
    core_url: &str,
    registration: &crate::ServiceRegistration,
) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let url = format!("{}/services/register", core_url);
    let mut delay = Duration::from_secs(2);
    let max_delay = Duration::from_secs(60);

    loop {
        match client.post(&url).json(registration).send().await {
            Ok(r) if r.status().is_success() => {
                tracing::info!("Registered with Core successfully");
                return;
            }
            Ok(r) => tracing::warn!(
                "Registration failed: HTTP {}, retrying in {:?}",
                r.status(),
                delay
            ),
            Err(e) => tracing::warn!("Core unreachable: {}, retrying in {:?}", e, delay),
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(max_delay);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_retry_succeeds_on_first_attempt() {
        let result = retry_with_backoff(3, Duration::from_millis(1), || async {
            Ok::<i32, String>(42)
        })
        .await;

        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn test_retry_succeeds_on_second_attempt() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = retry_with_backoff(3, Duration::from_millis(1), || {
            let attempts = attempts_clone.clone();
            async move {
                let count = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                if count < 2 {
                    Err::<i32, String>(format!("Attempt {}", count))
                } else {
                    Ok(99)
                }
            }
        })
        .await;

        assert_eq!(result, Ok(99));
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_exhausts_attempts() {
        let result =
            retry_with_backoff::<_, _, i32, String>(3, Duration::from_millis(1), || async {
                Err("Always fails".to_string())
            })
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_retry_exponential_backoff() {
        use std::time::Instant;

        let start = Instant::now();
        let result =
            retry_with_backoff::<_, _, i32, String>(3, Duration::from_millis(10), || async {
                Err("Always fails".to_string())
            })
            .await;

        let elapsed = start.elapsed();
        assert!(result.is_err());
        assert!(
            elapsed.as_millis() >= 25,
            "Expected at least 25ms delay for exponential backoff, got {}ms",
            elapsed.as_millis()
        );
    }

    #[tokio::test]
    async fn test_retry_success_after_multiple_failures() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = retry_with_backoff(5, Duration::from_millis(1), || {
            let attempts = attempts_clone.clone();
            async move {
                let count = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                if count < 4 {
                    Err::<i32, String>(format!("Attempt {}", count))
                } else {
                    Ok(123)
                }
            }
        })
        .await;

        assert_eq!(result, Ok(123));
        assert_eq!(attempts.load(Ordering::SeqCst), 4);
    }
}

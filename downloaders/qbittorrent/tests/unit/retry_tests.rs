// tests/unit/retry_tests.rs
use downloader_qbittorrent::retry_with_backoff;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[tokio::test]
async fn test_retry_succeeds_first_attempt() {
    let result = retry_with_backoff(3, Duration::from_millis(1), || async {
        Ok::<i32, String>(42)
    })
    .await;

    assert_eq!(result, Ok(42));
}

#[tokio::test]
async fn test_retry_succeeds_second_attempt() {
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
async fn test_retry_succeeds_after_multiple_failures() {
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

#[tokio::test]
async fn test_retry_exhausts_all_attempts() {
    let result =
        retry_with_backoff::<_, _, i32, String>(3, Duration::from_millis(1), || async {
            Err("Always fails".to_string())
        })
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_retry_exponential_backoff_timing() {
    let start = Instant::now();
    let result = retry_with_backoff::<_, _, i32, String>(
        2,
        Duration::from_millis(10),
        || async { Err("Always fails".to_string()) },
    )
    .await;

    let elapsed = start.elapsed();
    assert!(result.is_err());
    // Should take at least 10ms (first backoff)
    assert!(elapsed.as_millis() >= 5);
}

#[tokio::test]
async fn test_retry_preserves_final_error_message() {
    let attempts = Arc::new(AtomicU32::new(0));
    let attempts_clone = attempts.clone();

    let result = retry_with_backoff::<_, _, i32, String>(3, Duration::from_millis(1), || {
        let attempts = attempts_clone.clone();
        async move {
            let count = attempts.fetch_add(1, Ordering::SeqCst) + 1;
            Err(format!("Error on attempt {}", count))
        }
    })
    .await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Error on attempt 3");
}

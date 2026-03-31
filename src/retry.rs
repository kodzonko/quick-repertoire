use std::future::Future;
use std::time::Duration;

use tokio::time::sleep;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    max_attempts: usize,
    initial_delay: Duration,
    max_delay: Duration,
}

impl RetryPolicy {
    pub const fn new(max_attempts: usize, initial_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_attempts: if max_attempts == 0 { 1 } else { max_attempts },
            initial_delay,
            max_delay,
        }
    }

    pub const fn network_requests() -> Self {
        Self::new(4, Duration::from_millis(250), Duration::from_secs(2))
    }

    fn delay_for_retry(self, attempt: usize) -> Duration {
        let exponent = attempt.saturating_sub(1).min(31) as u32;
        let multiplier = 1_u32.checked_shl(exponent).unwrap_or(u32::MAX);
        self.initial_delay.saturating_mul(multiplier).min(self.max_delay)
    }
}

pub enum RetryDirective<E> {
    Fail(E),
    Retry(E),
    RetryAfter { error: E, delay: Duration },
}

impl<E> RetryDirective<E> {
    pub fn fail(error: E) -> Self {
        Self::Fail(error)
    }

    pub fn retry(error: E) -> Self {
        Self::Retry(error)
    }

    pub fn retry_after(error: E, delay: Duration) -> Self {
        Self::RetryAfter { error, delay }
    }
}

pub async fn retry_with_backoff<T, E, O, Fut>(policy: RetryPolicy, mut operation: O) -> Result<T, E>
where
    O: FnMut(usize) -> Fut,
    Fut: Future<Output = Result<T, RetryDirective<E>>>,
{
    for attempt in 1..=policy.max_attempts {
        match operation(attempt).await {
            Ok(value) => return Ok(value),
            Err(RetryDirective::Fail(error)) => return Err(error),
            Err(RetryDirective::Retry(error)) => {
                if attempt == policy.max_attempts {
                    return Err(error);
                }
                sleep(policy.delay_for_retry(attempt)).await;
            }
            Err(RetryDirective::RetryAfter { error, delay }) => {
                if attempt == policy.max_attempts {
                    return Err(error);
                }
                sleep(delay.min(policy.max_delay)).await;
            }
        }
    }

    unreachable!("retry loop must return or exhaust the configured attempts");
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn retry_policy_caps_exponential_backoff() {
        let policy = RetryPolicy::new(4, Duration::from_millis(100), Duration::from_millis(250));

        assert_eq!(policy.delay_for_retry(1), Duration::from_millis(100));
        assert_eq!(policy.delay_for_retry(2), Duration::from_millis(200));
        assert_eq!(policy.delay_for_retry(3), Duration::from_millis(250));
        assert_eq!(policy.delay_for_retry(4), Duration::from_millis(250));
    }

    #[tokio::test]
    async fn retry_with_backoff_retries_until_the_operation_succeeds() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let tracked_attempts = attempts.clone();

        let result =
            retry_with_backoff(RetryPolicy::new(3, Duration::ZERO, Duration::ZERO), move |_| {
                let tracked_attempts = tracked_attempts.clone();
                async move {
                    let current_attempt = tracked_attempts.fetch_add(1, Ordering::SeqCst);
                    if current_attempt == 0 {
                        Err(RetryDirective::retry("temporary failure"))
                    } else {
                        Ok("success")
                    }
                }
            })
            .await;

        assert_eq!(result, Ok("success"));
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn retry_with_backoff_stops_on_non_retryable_errors() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let tracked_attempts = attempts.clone();

        let result =
            retry_with_backoff(RetryPolicy::new(3, Duration::ZERO, Duration::ZERO), move |_| {
                let tracked_attempts = tracked_attempts.clone();
                async move {
                    tracked_attempts.fetch_add(1, Ordering::SeqCst);
                    Err::<(), _>(RetryDirective::fail("permanent failure"))
                }
            })
            .await;

        assert_eq!(result, Err("permanent failure"));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
}

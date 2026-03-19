//! Retry with exponential backoff and idempotency key generation.

use serde::{Deserialize, Serialize};
use std::time::Duration;

// ===== Error Categories =====

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Timeout,
    RateLimit,
    ServerError,
    AuthError,
    ValidationError,
    PolicyDeny,
    Unknown,
}

impl ErrorCategory {
    /// Classify an API error string into a category.
    pub fn classify(error: &str) -> Self {
        let lower = error.to_lowercase();

        if lower.contains("timeout") || lower.contains("timed out") {
            return ErrorCategory::Timeout;
        }
        if lower.contains("429") || lower.contains("rate limit") || lower.contains("too many") {
            return ErrorCategory::RateLimit;
        }
        if lower.contains("500") || lower.contains("502") || lower.contains("503")
            || lower.contains("server error") || lower.contains("internal error")
        {
            return ErrorCategory::ServerError;
        }
        if lower.contains("401") || lower.contains("403")
            || lower.contains("unauthorized") || lower.contains("forbidden")
            || lower.contains("invalid api key") || lower.contains("auth")
        {
            return ErrorCategory::AuthError;
        }
        if lower.contains("400") || lower.contains("validation")
            || lower.contains("invalid") || lower.contains("malformed")
        {
            return ErrorCategory::ValidationError;
        }
        if lower.contains("policy") || lower.contains("denied") || lower.contains("blocked") {
            return ErrorCategory::PolicyDeny;
        }

        ErrorCategory::Unknown
    }

    /// Whether this error category is retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ErrorCategory::Timeout | ErrorCategory::RateLimit | ErrorCategory::ServerError
        )
    }
}

// ===== Retry Policy =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
        }
    }
}

impl RetryPolicy {
    /// Calculate delay for a given attempt (0-indexed).
    /// Uses exponential backoff: base * 2^attempt, capped at max_delay.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay_ms = self
            .base_delay_ms
            .saturating_mul(1u64 << attempt.min(10))
            .min(self.max_delay_ms);
        Duration::from_millis(delay_ms)
    }

    /// Whether another retry should be attempted.
    pub fn should_retry(&self, attempt: u32, error: &str) -> bool {
        if attempt >= self.max_attempts {
            return false;
        }
        let category = ErrorCategory::classify(error);
        category.is_retryable()
    }
}

// ===== Idempotency Key =====

/// Generate an idempotency key from task/step context.
/// Key = sha256(task_id + step_index + agent_id)
pub fn idempotency_key(task_id: &str, step_index: u32, agent_id: &str) -> String {
    use sha2::{Digest, Sha256};
    let input = format!("{}:{}:{}", task_id, step_index, agent_id);
    let hash = Sha256::digest(input.as_bytes());
    format!("sha256:{:x}", hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- ErrorCategory::classify ---

    #[test]
    fn test_classify_timeout() {
        assert_eq!(ErrorCategory::classify("Connection timed out after 30s"), ErrorCategory::Timeout);
        assert_eq!(ErrorCategory::classify("Request timeout"), ErrorCategory::Timeout);
    }

    #[test]
    fn test_classify_rate_limit() {
        assert_eq!(ErrorCategory::classify("HTTP 429 Too Many Requests"), ErrorCategory::RateLimit);
        assert_eq!(ErrorCategory::classify("Rate limit exceeded"), ErrorCategory::RateLimit);
    }

    #[test]
    fn test_classify_server_error() {
        assert_eq!(ErrorCategory::classify("HTTP 500 Internal Server Error"), ErrorCategory::ServerError);
        assert_eq!(ErrorCategory::classify("502 Bad Gateway"), ErrorCategory::ServerError);
        assert_eq!(ErrorCategory::classify("503 Service Unavailable"), ErrorCategory::ServerError);
    }

    #[test]
    fn test_classify_auth_error() {
        assert_eq!(ErrorCategory::classify("HTTP 401 Unauthorized"), ErrorCategory::AuthError);
        assert_eq!(ErrorCategory::classify("Invalid API key"), ErrorCategory::AuthError);
        assert_eq!(ErrorCategory::classify("403 Forbidden"), ErrorCategory::AuthError);
    }

    #[test]
    fn test_classify_validation_error() {
        assert_eq!(ErrorCategory::classify("400 Bad Request: malformed JSON"), ErrorCategory::ValidationError);
        assert_eq!(ErrorCategory::classify("Validation failed"), ErrorCategory::ValidationError);
    }

    #[test]
    fn test_classify_policy_deny() {
        assert_eq!(ErrorCategory::classify("Policy denied this action"), ErrorCategory::PolicyDeny);
        assert_eq!(ErrorCategory::classify("Command blocked by guardrails"), ErrorCategory::PolicyDeny);
    }

    #[test]
    fn test_classify_unknown() {
        assert_eq!(ErrorCategory::classify("Something weird happened"), ErrorCategory::Unknown);
    }

    // --- is_retryable ---

    #[test]
    fn test_retryable_categories() {
        assert!(ErrorCategory::Timeout.is_retryable());
        assert!(ErrorCategory::RateLimit.is_retryable());
        assert!(ErrorCategory::ServerError.is_retryable());
    }

    #[test]
    fn test_non_retryable_categories() {
        assert!(!ErrorCategory::AuthError.is_retryable());
        assert!(!ErrorCategory::ValidationError.is_retryable());
        assert!(!ErrorCategory::PolicyDeny.is_retryable());
        assert!(!ErrorCategory::Unknown.is_retryable());
    }

    // --- RetryPolicy ---

    #[test]
    fn test_default_retry_policy() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_attempts, 3);
        assert_eq!(policy.base_delay_ms, 1000);
        assert_eq!(policy.max_delay_ms, 30000);
    }

    #[test]
    fn test_delay_exponential_backoff() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(1000));
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(2000));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(4000));
        assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(8000));
        assert_eq!(policy.delay_for_attempt(4), Duration::from_millis(16000));
    }

    #[test]
    fn test_delay_capped_at_max() {
        let policy = RetryPolicy {
            max_attempts: 10,
            base_delay_ms: 1000,
            max_delay_ms: 5000,
        };
        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(1000));
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(2000));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(4000));
        assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(5000)); // capped
        assert_eq!(policy.delay_for_attempt(10), Duration::from_millis(5000)); // capped
    }

    #[test]
    fn test_should_retry_retryable_error() {
        let policy = RetryPolicy::default();
        assert!(policy.should_retry(0, "Connection timed out"));
        assert!(policy.should_retry(1, "HTTP 429 Too Many Requests"));
        assert!(policy.should_retry(2, "500 Internal Server Error"));
    }

    #[test]
    fn test_should_not_retry_non_retryable() {
        let policy = RetryPolicy::default();
        assert!(!policy.should_retry(0, "401 Unauthorized"));
        assert!(!policy.should_retry(0, "Policy denied"));
        assert!(!policy.should_retry(0, "400 Bad Request"));
    }

    #[test]
    fn test_should_not_retry_max_attempts() {
        let policy = RetryPolicy::default();
        assert!(!policy.should_retry(3, "Connection timed out"));
        assert!(!policy.should_retry(5, "500 Internal Server Error"));
    }

    // --- Idempotency Key ---

    #[test]
    fn test_idempotency_key_deterministic() {
        let k1 = idempotency_key("task-1", 0, "ceo");
        let k2 = idempotency_key("task-1", 0, "ceo");
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_idempotency_key_differs_by_step() {
        let k1 = idempotency_key("task-1", 0, "ceo");
        let k2 = idempotency_key("task-1", 1, "ceo");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_idempotency_key_differs_by_agent() {
        let k1 = idempotency_key("task-1", 0, "ceo");
        let k2 = idempotency_key("task-1", 0, "devops");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_idempotency_key_differs_by_task() {
        let k1 = idempotency_key("task-1", 0, "ceo");
        let k2 = idempotency_key("task-2", 0, "ceo");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_idempotency_key_format() {
        let key = idempotency_key("t", 0, "a");
        assert!(key.starts_with("sha256:"));
        assert!(key.len() > 10);
    }
}

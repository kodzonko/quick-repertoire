use std::sync::OnceLock;

use env_logger::Env;

static LOGGING_INIT: OnceLock<()> = OnceLock::new();

const DEFAULT_LOG_FILTER: &str = "error";

pub fn init_logging() {
    LOGGING_INIT.get_or_init(|| {
        let mut builder =
            env_logger::Builder::from_env(Env::default().default_filter_or(DEFAULT_LOG_FILTER));
        builder.format_timestamp_secs();
        let _ = builder.try_init();
    });
}

pub fn preview_for_log(value: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let total_chars = value.chars().count();
    if total_chars <= max_chars {
        return value.to_string();
    }

    let truncated = value.chars().take(max_chars).collect::<String>();
    format!("{truncated}… ({total_chars} chars total)")
}

pub fn redact_secret(secret: &str) -> String {
    let total_chars = secret.chars().count();
    if total_chars == 0 {
        return "<empty>".to_string();
    }

    if total_chars <= 8 {
        return format!("*** (len={total_chars})");
    }

    let prefix = secret.chars().take(4).collect::<String>();
    let suffix = secret.chars().skip(total_chars.saturating_sub(4)).collect::<String>();
    format!("{prefix}***{suffix} (len={total_chars})")
}

pub async fn response_body_preview(response: reqwest::Response, max_chars: usize) -> String {
    match response.text().await {
        Ok(body) if body.trim().is_empty() => "<empty>".to_string(),
        Ok(body) => preview_for_log(&body, max_chars),
        Err(error) => format!("<unavailable: {error}>"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_for_log_truncates_long_values() {
        assert_eq!(preview_for_log("abcdefgh", 5), "abcde… (8 chars total)");
    }

    #[test]
    fn redact_secret_masks_short_and_long_values() {
        assert_eq!(redact_secret(""), "<empty>");
        assert_eq!(redact_secret("abcd1234"), "*** (len=8)");
        assert_eq!(redact_secret("1234567890abcdef"), "1234***cdef (len=16)");
    }
}

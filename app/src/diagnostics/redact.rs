//! Redaction helpers. Secrets must never reach logs, errors, or debug output.

use std::fmt;

/// Placeholder string used anywhere a secret would otherwise be printed.
pub const REDACTED: &str = "<redacted>";

/// Render a secret as a fixed placeholder that also records its length.
///
/// Length is safe to log for diagnostics and is useful for spotting
/// truncation, unlike the secret value itself.
pub fn redact_secret(value: &str) -> String {
    if value.is_empty() {
        "<empty>".to_string()
    } else {
        format!("<redacted:{} chars>", value.chars().count())
    }
}

/// Mask a stream key for on-screen display, revealing only the last 4 chars.
pub fn mask_stream_key(key: &str) -> String {
    let n = key.chars().count();
    match n {
        0 => String::new(),
        1..=4 => "*".repeat(n),
        _ => {
            let visible: String = key.chars().skip(n - 4).collect();
            format!("{}{visible}", "*".repeat(n - 4))
        }
    }
}

/// A `Display` wrapper that prints the redacted form of a secret.
pub struct Redacted<'a>(pub &'a str);

impl fmt::Display for Redacted<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&redact_secret(self.0))
    }
}

impl fmt::Debug for Redacted<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_never_contains_secret() {
        let secret = "super-secret-token-value";
        let out = redact_secret(secret);
        assert!(!out.contains(secret));
        assert_eq!(out, "<redacted:24 chars>");
    }

    #[test]
    fn redact_empty() {
        assert_eq!(redact_secret(""), "<empty>");
    }

    #[test]
    fn mask_stream_key_reveals_only_tail() {
        assert_eq!(mask_stream_key("abcd1234"), "****1234");
        assert_eq!(mask_stream_key("abc"), "***");
        assert_eq!(mask_stream_key(""), "");
        let masked = mask_stream_key("abcdef");
        assert!(!masked.contains("abc"));
    }

    #[test]
    fn redacted_wrapper_hides_value_in_debug() {
        let secret = "tok_1234567890";
        let rendered = format!("{:?}", Redacted(secret));
        assert!(!rendered.contains(secret));
    }
}

/// Secret redaction utilities ensuring tokens, credentials, and API keys are never exposed.
pub struct SecretRedactor;

impl SecretRedactor {
    /// Redacts known secret patterns from a generic text string.
    pub fn redact_text(input: &str) -> String {
        let mut output = input.to_string();

        // OpenAI keys (sk-...)
        output = Self::replace_pattern(&output, "sk-", 20);
        // Groq keys (gsk_...)
        output = Self::replace_pattern(&output, "gsk_", 20);
        // GitHub tokens (ghp_..., gho_...)
        output = Self::replace_pattern(&output, "ghp_", 20);
        output = Self::replace_pattern(&output, "gho_", 20);
        // Anthropic keys (sk-ant-...)
        output = Self::replace_pattern(&output, "sk-ant-", 20);

        output
    }

    /// Redacts an environment variable value if its key suggests a secret or sensitive credential.
    pub fn redact_env_var(key: &str, value: &str) -> String {
        let upper_key = key.to_uppercase();

        if upper_key.contains("API_KEY")
            || upper_key.contains("SECRET")
            || upper_key.contains("PASSWORD")
            || upper_key.contains("TOKEN")
            || upper_key.contains("AUTH")
            || upper_key.contains("BEARER")
            || upper_key.contains("PRIVATE")
            || upper_key.contains("CREDENTIAL")
            || upper_key.contains("DATABASE_URL")
            || upper_key.contains("CONN_STR")
            || upper_key == "HADES_API_KEY"
            || upper_key == "OPENAI_API_KEY"
            || upper_key == "GROQ_API_KEY"
            || upper_key == "ANTHROPIC_API_KEY"
        {
            if value.is_empty() {
                String::new()
            } else if value.len() > 8 {
                format!("{}...[REDACTED]", &value[..4])
            } else {
                "[REDACTED]".to_string()
            }
        } else {
            Self::redact_text(value)
        }
    }

    fn replace_pattern(text: &str, prefix: &str, min_secret_len: usize) -> String {
        let mut result = String::with_capacity(text.len());
        let mut remaining = text;

        while let Some(start_idx) = remaining.find(prefix) {
            result.push_str(&remaining[..start_idx]);
            let secret_candidate = &remaining[start_idx..];

            // Find end of token (delimiter: whitespace, quote, comma, semicolon, newline)
            let end_idx = secret_candidate
                .find(|c: char| {
                    c.is_whitespace()
                        || c == '"'
                        || c == '\''
                        || c == ','
                        || c == ';'
                        || c == ')'
                        || c == ']'
                })
                .unwrap_or(secret_candidate.len());

            let token = &secret_candidate[..end_idx];
            if token.len() >= prefix.len() + min_secret_len {
                result.push_str(&format!("{}[REDACTED]", prefix));
            } else {
                result.push_str(token);
            }

            remaining = &secret_candidate[end_idx..];
        }

        result.push_str(remaining);
        result
    }
}

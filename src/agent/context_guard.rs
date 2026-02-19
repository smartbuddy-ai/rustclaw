//! Feature 6: Context window overflow guard.
//! Truncates tool output if the total message size approaches the model's context window.

/// Known context window sizes (in tokens) for common models.
pub fn model_context_window(model: &str) -> usize {
    if model.contains("context-1m") || model.contains("1m") {
        1_000_000
    } else if model.contains("claude") {
        200_000
    } else if model.contains("gpt-4o") || model.contains("gpt-4-turbo") || model.contains("gpt-4-0125") {
        128_000
    } else if model.contains("gpt-4") {
        8_192
    } else if model.contains("gpt-3.5") {
        16_385
    } else {
        // Conservative default
        100_000
    }
}

/// Approximate token count from character count (chars / 4).
pub fn approx_tokens(text: &str) -> usize {
    text.len() / 4
}

/// Truncate tool output if the total context would exceed the model's window.
/// Returns the (possibly truncated) output and a flag indicating if truncation occurred.
pub fn guard_context_overflow(
    existing_messages_chars: usize,
    tool_output: &str,
    model: &str,
) -> (String, bool) {
    let context_window = model_context_window(model);
    let threshold = (context_window as f64 * 0.8) as usize; // 80% threshold

    let existing_tokens = existing_messages_chars / 4;
    let output_tokens = approx_tokens(tool_output);
    let total = existing_tokens + output_tokens;

    if total <= threshold {
        return (tool_output.to_string(), false);
    }

    // Compute how many tokens we can afford for the output
    let available_tokens = threshold.saturating_sub(existing_tokens);
    let available_chars = available_tokens * 4;

    if available_chars == 0 {
        tracing::warn!(
            model,
            existing_tokens,
            output_tokens,
            "context window full, dropping tool output entirely"
        );
        return ("... [truncated: context window full]".to_string(), true);
    }

    let truncated = if available_chars < tool_output.len() {
        // Find a safe UTF-8 boundary
        let end = tool_output
            .char_indices()
            .take_while(|(i, _)| *i < available_chars)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        format!("{}... [truncated: {} chars removed to fit context window]", &tool_output[..end], tool_output.len() - end)
    } else {
        tool_output.to_string()
    };

    tracing::warn!(
        model,
        existing_tokens,
        output_tokens,
        truncated_to = approx_tokens(&truncated),
        "tool output truncated to fit context window"
    );

    (truncated, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_context_windows() {
        assert_eq!(model_context_window("claude-sonnet-4-20250514"), 200_000);
        assert_eq!(model_context_window("claude-opus-4-6"), 200_000);
        assert_eq!(model_context_window("gpt-4o"), 128_000);
        assert_eq!(model_context_window("gpt-3.5-turbo"), 16_385);
        assert_eq!(model_context_window("unknown-model"), 100_000);
    }

    #[test]
    fn no_truncation_when_under_threshold() {
        let output = "x".repeat(1000);
        let (result, truncated) = guard_context_overflow(0, &output, "claude-sonnet-4-20250514");
        assert!(!truncated);
        assert_eq!(result, output);
    }

    #[test]
    fn truncation_when_over_threshold() {
        // claude-sonnet has 200k tokens = 800k chars. 80% threshold = 640k chars.
        let existing = 600_000; // already 600k chars used
        let output = "x".repeat(200_000); // 200k more -> 800k total, above 80% threshold
        let (result, truncated) = guard_context_overflow(existing, &output, "claude-sonnet-4-20250514");
        assert!(truncated);
        assert!(result.len() < output.len());
        assert!(result.contains("[truncated"));
    }

    #[test]
    fn full_context_drops_output() {
        // Existing messages already at 80% of context
        let window = model_context_window("claude-sonnet-4-20250514"); // 200k tokens
        let threshold_chars = (window as f64 * 0.8) as usize * 4;
        let (result, truncated) = guard_context_overflow(threshold_chars + 1000, "some output", "claude-sonnet-4-20250514");
        assert!(truncated);
        assert!(result.contains("context window full"));
    }

    #[test]
    fn approx_tokens_works() {
        assert_eq!(approx_tokens("hello world!"), 3); // 12 chars / 4 = 3
        assert_eq!(approx_tokens(""), 0);
    }
}

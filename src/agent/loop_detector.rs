//! Feature 7: Tool loop detection.
//! Circuit breaker against repetitive tool call loops.

use std::collections::hash_map::DefaultHasher;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

/// Maximum history size to keep.
const HISTORY_SIZE: usize = 20;

/// Threshold for warning injection.
const WARN_THRESHOLD: usize = 5;

/// Threshold for circuit breaker (force final response).
const BREAK_THRESHOLD: usize = 10;

/// Result of checking for a tool loop.
#[derive(Debug, PartialEq)]
pub enum LoopAction {
    /// No loop detected, continue normally.
    Continue,
    /// Loop detected but below circuit breaker threshold. Inject a warning.
    Warn(String),
    /// Circuit breaker triggered. Force a final response.
    Break(String),
}

/// Tracks recent tool calls to detect loops.
pub struct LoopDetector {
    history: VecDeque<u64>,
}

impl LoopDetector {
    pub fn new() -> Self {
        Self {
            history: VecDeque::with_capacity(HISTORY_SIZE),
        }
    }

    /// Record a tool call and check for loops.
    pub fn record(&mut self, tool_name: &str, args: &serde_json::Value) -> LoopAction {
        let hash = Self::compute_hash(tool_name, args);

        if self.history.len() >= HISTORY_SIZE {
            self.history.pop_front();
        }
        self.history.push_back(hash);

        // Count consecutive identical calls from the end
        let consecutive = self
            .history
            .iter()
            .rev()
            .take_while(|&&h| h == hash)
            .count();

        if consecutive >= BREAK_THRESHOLD {
            LoopAction::Break(format!(
                "CIRCUIT BREAKER: Tool '{tool_name}' called {consecutive} times consecutively with same arguments. \
                 You must provide a final response now without calling any more tools."
            ))
        } else if consecutive >= WARN_THRESHOLD {
            LoopAction::Warn(format!(
                "WARNING: Tool '{tool_name}' has been called {consecutive} times consecutively with the same arguments. \
                 Consider a different approach or provide a final response."
            ))
        } else {
            LoopAction::Continue
        }
    }

    /// Reset the history.
    pub fn reset(&mut self) {
        self.history.clear();
    }

    fn compute_hash(tool_name: &str, args: &serde_json::Value) -> u64 {
        let mut hasher = DefaultHasher::new();
        tool_name.hash(&mut hasher);
        // Normalize JSON by converting to string
        let args_str = args.to_string();
        args_str.hash(&mut hasher);
        hasher.finish()
    }
}

impl Default for LoopDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn no_loop_detected_initially() {
        let mut detector = LoopDetector::new();
        let action = detector.record("web_search", &json!({"query": "rust"}));
        assert_eq!(action, LoopAction::Continue);
    }

    #[test]
    fn no_loop_with_different_calls() {
        let mut detector = LoopDetector::new();
        for i in 0..10 {
            let action = detector.record("web_search", &json!({"query": format!("query{i}")}));
            assert_eq!(action, LoopAction::Continue);
        }
    }

    #[test]
    fn warn_after_threshold() {
        let mut detector = LoopDetector::new();
        let args = json!({"query": "same thing"});
        for _ in 0..4 {
            assert_eq!(detector.record("web_search", &args), LoopAction::Continue);
        }
        // 5th call should trigger warning
        match detector.record("web_search", &args) {
            LoopAction::Warn(msg) => {
                assert!(msg.contains("5 times"));
                assert!(msg.contains("web_search"));
            }
            other => panic!("expected Warn, got {:?}", other),
        }
    }

    #[test]
    fn break_after_threshold() {
        let mut detector = LoopDetector::new();
        let args = json!({"query": "stuck"});
        for _ in 0..9 {
            let _ = detector.record("web_search", &args);
        }
        // 10th call should trigger circuit breaker
        match detector.record("web_search", &args) {
            LoopAction::Break(msg) => {
                assert!(msg.contains("CIRCUIT BREAKER"));
                assert!(msg.contains("10 times"));
            }
            other => panic!("expected Break, got {:?}", other),
        }
    }

    #[test]
    fn different_tool_resets_consecutive_count() {
        let mut detector = LoopDetector::new();
        let args = json!({"query": "same"});
        for _ in 0..4 {
            detector.record("web_search", &args);
        }
        // Different tool breaks the chain
        detector.record("file_read", &json!({"path": "/tmp/file"}));
        // Same tool again should start fresh count
        assert_eq!(detector.record("web_search", &args), LoopAction::Continue);
    }

    #[test]
    fn reset_clears_history() {
        let mut detector = LoopDetector::new();
        let args = json!({"query": "same"});
        for _ in 0..4 {
            detector.record("web_search", &args);
        }
        detector.reset();
        assert_eq!(detector.record("web_search", &args), LoopAction::Continue);
    }
}

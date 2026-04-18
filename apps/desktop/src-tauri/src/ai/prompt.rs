// 5-layer context assembly: BASE > SNAPSHOT > INTENT > HISTORY > MEMORY — T-022
// BASE and SNAPSHOT go into the system prompt and are never trimmed.
// INTENT, HISTORY, and MEMORY share a mutable token budget; HISTORY is trimmed
// oldest-first when over budget, then MEMORY is dropped if still over.

use crate::ai::payee_memory::MemoryHint;
use crate::ai::{BuiltPrompt, Message};
use crate::ai::classifier::ClassifiedIntent;

const DEFAULT_TOKEN_BUDGET: usize = 6_000;

fn approx_tokens(s: &str) -> usize {
    (s.len() / 4).max(1)
}

pub struct PromptBuilder {
    base: String,
    snapshot: String,
    intent: Option<ClassifiedIntent>,
    history: Vec<Message>,
    memory_hints: Vec<MemoryHint>,
    token_budget: usize,
}

impl PromptBuilder {
    pub fn new(base: impl Into<String>, snapshot: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            snapshot: snapshot.into(),
            intent: None,
            history: Vec::new(),
            memory_hints: Vec::new(),
            token_budget: DEFAULT_TOKEN_BUDGET,
        }
    }

    pub fn with_intent(mut self, intent: ClassifiedIntent) -> Self {
        self.intent = Some(intent);
        self
    }

    pub fn with_history(mut self, history: Vec<Message>) -> Self {
        self.history = history;
        self
    }

    pub fn with_memory(mut self, hints: Vec<MemoryHint>) -> Self {
        self.memory_hints = hints;
        self
    }

    pub fn with_token_budget(mut self, budget: usize) -> Self {
        self.token_budget = budget;
        self
    }

    pub fn build(self) -> BuiltPrompt {
        let system = format!("{}\n\n---\n\n{}", self.base, self.snapshot);

        let mut messages: Vec<Message> = Vec::new();
        let mut used: usize = 0;

        // INTENT — always include; never trimmed.
        if let Some(intent) = &self.intent {
            let text = format!("[Intent: {:?}]", intent.kind);
            used += approx_tokens(&text);
            messages.push(Message::user(text));
        }

        // HISTORY — fill remaining budget after intent, oldest dropped first.
        let history_budget = self.token_budget.saturating_sub(used);
        let trimmed = trim_to_budget(self.history, history_budget);
        used += trimmed.iter().map(|m| approx_tokens(&m.content)).sum::<usize>();
        messages.extend(trimmed);

        // MEMORY — append only if budget remains after intent + history.
        let memory_text = format_memory_hints(&self.memory_hints);
        if !memory_text.is_empty() {
            let memory_tokens = approx_tokens(&memory_text);
            if used + memory_tokens <= self.token_budget {
                messages.push(Message::user(format!("[Payee memory:\n{}]", memory_text)));
            }
        }

        BuiltPrompt { system, messages }
    }
}

fn format_memory_hints(hints: &[MemoryHint]) -> String {
    if hints.is_empty() {
        return String::new();
    }
    hints
        .iter()
        .map(|h| format!("  \"{}\" → {}", h.payee_name, h.account_id))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Keeps the newest messages that fit within `budget` tokens.
/// The most-recent message is always kept regardless of size.
fn trim_to_budget(history: Vec<Message>, budget: usize) -> Vec<Message> {
    if history.is_empty() {
        return history;
    }
    let mut kept: Vec<Message> = Vec::new();
    let mut total: usize = 0;
    for msg in history.into_iter().rev() {
        let tokens = approx_tokens(&msg.content);
        if total + tokens > budget && !kept.is_empty() {
            break;
        }
        total += tokens;
        kept.push(msg);
    }
    kept.reverse();
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::classifier::IntentKind;

    #[test]
    fn system_contains_base() {
        let p = PromptBuilder::new("BASE TEXT", "SNAP").build();
        assert!(p.system.contains("BASE TEXT"));
    }

    #[test]
    fn system_contains_snapshot() {
        let p = PromptBuilder::new("BASE", "SNAP TEXT").build();
        assert!(p.system.contains("SNAP TEXT"));
    }

    #[test]
    fn intent_becomes_first_message() {
        let intent = ClassifiedIntent { kind: IntentKind::RecordExpense };
        let p = PromptBuilder::new("B", "S").with_intent(intent).build();
        assert!(!p.messages.is_empty());
        assert!(p.messages[0].content.contains("RecordExpense"));
    }

    #[test]
    fn history_messages_appear_after_intent() {
        let intent = ClassifiedIntent { kind: IntentKind::QueryBalance };
        let history = vec![Message::user("hello"), Message::assistant("hi")];
        let p = PromptBuilder::new("B", "S")
            .with_intent(intent)
            .with_history(history)
            .build();
        // intent first, then history
        assert!(p.messages[0].content.contains("QueryBalance"));
        assert_eq!(p.messages[1].content, "hello");
        assert_eq!(p.messages[2].content, "hi");
    }

    #[test]
    fn history_trimmed_oldest_first_when_over_budget() {
        // 3 messages × 500 tokens each = 1500; budget = 800 → only 1 fits
        let history = vec![
            Message::user("a".repeat(500 * 4)),
            Message::user("b".repeat(500 * 4)),
            Message::user("c".repeat(500 * 4)),
        ];
        let p = PromptBuilder::new("B", "S")
            .with_history(history)
            .with_token_budget(800)
            .build();
        // Only the most-recent message should remain
        assert_eq!(p.messages.len(), 1);
        assert!(p.messages[0].content.starts_with('c'));
    }

    #[test]
    fn most_recent_message_always_kept() {
        // Budget so small only one message fits; newest must survive.
        let history = vec![
            Message::user("old message"),
            Message::user("new message"),
        ];
        let p = PromptBuilder::new("B", "S")
            .with_history(history)
            .with_token_budget(10)
            .build();
        assert!(p.messages.iter().any(|m| m.content == "new message"));
    }

    #[test]
    fn memory_hints_appended_after_history() {
        let hints = vec![MemoryHint {
            payee_name: "Whole Foods".to_string(),
            account_id: "acc_groceries".to_string(),
            use_count: 5,
        }];
        let history = vec![Message::user("test")];
        let p = PromptBuilder::new("B", "S")
            .with_history(history)
            .with_memory(hints)
            .build();
        let last = p.messages.last().unwrap();
        assert!(last.content.contains("Payee memory"));
        assert!(last.content.contains("Whole Foods"));
    }

    #[test]
    fn memory_omitted_when_budget_exhausted() {
        let hints = vec![MemoryHint {
            payee_name: "Whole Foods".to_string(),
            account_id: "acc_groceries".to_string(),
            use_count: 1,
        }];
        // Budget so tiny nothing fits except the most-recent history message
        let history = vec![Message::user("x".repeat(400 * 4))];
        let p = PromptBuilder::new("B", "S")
            .with_memory(hints)
            .with_history(history)
            .with_token_budget(400)
            .build();
        assert!(!p.messages.iter().any(|m| m.content.contains("Payee memory")));
    }

    #[test]
    fn empty_builder_produces_empty_messages() {
        let p = PromptBuilder::new("B", "S").build();
        assert!(p.messages.is_empty());
    }

    #[test]
    fn all_history_kept_when_within_budget() {
        let history = vec![
            Message::user("a"),
            Message::assistant("b"),
            Message::user("c"),
        ];
        let p = PromptBuilder::new("B", "S").with_history(history).build();
        assert_eq!(p.messages.len(), 3);
    }
}

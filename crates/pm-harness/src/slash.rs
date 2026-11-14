//! The prompts that ask the harness for one of its conversation commands rather than for a turn, as the app tells them apart.

/// The prompt that answers the reader's last prompt again.
pub const RETRY: &str = "/retry";

/// The prompt that starts the conversation afresh.
pub const FRESH: &str = "/fresh";

/// The prompt that asks a question on the side, followed by the question.
pub const BTW: &str = "/btw";

/// The prompt that hands the work over to a new conversation, followed by what it should work toward, if anything.
pub const HANDOFF: &str = "/handoff";

/// The question `text` asks on the side, when it is `/btw` followed by one or by nothing; such a prompt starts no turn and is never queued, since the agent answers it while it works.
pub fn aside_question(text: &str) -> Option<&str> {
    let rest = text.trim().strip_prefix(BTW)?;
    match rest.chars().next() {
        None => Some(""),
        Some(first) if first.is_whitespace() => Some(rest.trim()),
        Some(_) => None,
    }
}

/// The prompt that hands the work over toward `goal`, or carries on with the same work when it is empty.
pub fn handoff_prompt(goal: &str) -> String {
    match goal.trim() {
        "" => HANDOFF.to_owned(),
        goal => format!("{HANDOFF} {goal}"),
    }
}

/// The prompt that asks `question` on the side.
pub fn btw_prompt(question: &str) -> String {
    format!("{BTW} {}", question.trim())
}

#[cfg(test)]
/// Tests for telling the conversation commands apart.
mod tests {
    use super::*;

    /// A `/btw` prompt gives its question, an empty one when it has none, and anything else, `/btwx` too, asks nothing on the side.
    #[test]
    fn a_btw_prompt_gives_its_question() {
        assert_eq!(aside_question("/btw why so slow? "), Some("why so slow?"));
        assert_eq!(aside_question(" /btw"), Some(""));
        assert_eq!(aside_question("/btwx why"), None);
        assert_eq!(aside_question("why /btw"), None);
        assert_eq!(
            aside_question(&btw_prompt(" which file? ")),
            Some("which file?")
        );
    }

    /// A hand-off prompt names its goal only when it has one.
    #[test]
    fn a_handoff_prompt_names_its_goal() {
        assert_eq!(handoff_prompt("  "), "/handoff");
        assert_eq!(handoff_prompt("ship it"), "/handoff ship it");
    }
}

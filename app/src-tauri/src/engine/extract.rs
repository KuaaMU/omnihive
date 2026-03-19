/// Pure extraction functions for parsing LLM responses.
///
/// All functions in this module are side-effect-free and easily testable.

/// Extract text between two markers, returning None if markers are missing or content is empty.
pub fn extract_between_markers(text: &str, start_marker: &str, end_marker: &str) -> Option<String> {
    let start_idx = text.find(start_marker)?;
    let content_start = start_idx + start_marker.len();
    let end_idx = text[content_start..].find(end_marker)?;
    let content = text[content_start..content_start + end_idx].trim();
    if content.is_empty() {
        None
    } else {
        Some(content.to_string())
    }
}

/// Extract consensus update from an LLM response.
/// Returns None if markers are missing or required sections are absent.
pub fn extract_consensus_update(response: &str) -> Option<String> {
    let content =
        extract_between_markers(response, "<<<CONSENSUS_START>>>", "<<<CONSENSUS_END>>>")?;

    if content.contains("## Company State")
        && content.contains("## Current Focus")
        && content.contains("## Decision Log")
        && content.len() > 100
    {
        Some(content)
    } else {
        None
    }
}

/// Extract reflection content from an LLM response.
pub fn extract_reflection(response: &str) -> Option<String> {
    extract_between_markers(response, "<<<REFLECTION_START>>>", "<<<REFLECTION_END>>>")
}

/// Extract handoff note from an LLM response.
pub fn extract_handoff(response: &str) -> Option<String> {
    extract_between_markers(response, "<<<HANDOFF_START>>>", "<<<HANDOFF_END>>>")
}

/// Extract skill request markers from an LLM response.
pub fn extract_skill_requests(response: &str) -> Vec<String> {
    let start = "<<<SKILL_REQUEST>>>";
    let end = "<<<SKILL_REQUEST_END>>>";
    let mut requests = Vec::new();

    let mut search_from = 0;
    while let Some(s_idx) = response[search_from..].find(start) {
        let abs_start = search_from + s_idx + start.len();
        if let Some(e_idx) = response[abs_start..].find(end) {
            let skill_name = response[abs_start..abs_start + e_idx].trim().to_string();
            if !skill_name.is_empty() {
                requests.push(skill_name);
            }
            search_from = abs_start + e_idx + end.len();
        } else {
            break;
        }
    }

    requests
}

/// Truncate a string to `max_len` characters, appending "..." if truncated.
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

/// Build the user prompt from consensus content and an optional handoff note.
pub fn build_user_prompt(consensus_content: &str, handoff_note: &str) -> String {
    if handoff_note.is_empty() {
        format!("Current consensus.md:\n\n{}", consensus_content)
    } else {
        format!(
            "## Handoff from Previous Agent\n\n{}\n\n---\n\nCurrent consensus.md:\n\n{}",
            handoff_note, consensus_content
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- extract_between_markers ---

    #[test]
    fn test_extract_between_markers_basic() {
        let text = "before<<<START>>>hello world<<<END>>>after";
        let result = extract_between_markers(text, "<<<START>>>", "<<<END>>>");
        assert_eq!(result, Some("hello world".to_string()));
    }

    #[test]
    fn test_extract_between_markers_with_whitespace() {
        let text = "<<<START>>>  trimmed content  <<<END>>>";
        let result = extract_between_markers(text, "<<<START>>>", "<<<END>>>");
        assert_eq!(result, Some("trimmed content".to_string()));
    }

    #[test]
    fn test_extract_between_markers_missing_start() {
        let text = "no markers here<<<END>>>";
        assert_eq!(extract_between_markers(text, "<<<START>>>", "<<<END>>>"), None);
    }

    #[test]
    fn test_extract_between_markers_missing_end() {
        let text = "<<<START>>>content without end";
        assert_eq!(extract_between_markers(text, "<<<START>>>", "<<<END>>>"), None);
    }

    #[test]
    fn test_extract_between_markers_empty_content() {
        let text = "<<<START>>>   <<<END>>>";
        assert_eq!(extract_between_markers(text, "<<<START>>>", "<<<END>>>"), None);
    }

    #[test]
    fn test_extract_between_markers_multiline() {
        let text = "<<<START>>>\nline 1\nline 2\n<<<END>>>";
        let result = extract_between_markers(text, "<<<START>>>", "<<<END>>>");
        assert_eq!(result, Some("line 1\nline 2".to_string()));
    }

    // --- extract_consensus_update ---

    #[test]
    fn test_extract_consensus_valid() {
        let response = r#"Some preamble
<<<CONSENSUS_START>>>
# Company Consensus

## Company State
We are building something.

## Current Focus
Shipping MVP.

## Decision Log
| Decision | Agent |
| --- | --- |
| Build X | CEO |

More content here to reach the 100 char minimum for validation.
<<<CONSENSUS_END>>>
Some postamble"#;
        let result = extract_consensus_update(response);
        assert!(result.is_some());
        let content = result.unwrap();
        assert!(content.contains("## Company State"));
        assert!(content.contains("## Current Focus"));
        assert!(content.contains("## Decision Log"));
    }

    #[test]
    fn test_extract_consensus_missing_sections() {
        let response = "<<<CONSENSUS_START>>>Only partial content without required sections<<<CONSENSUS_END>>>";
        assert_eq!(extract_consensus_update(response), None);
    }

    #[test]
    fn test_extract_consensus_no_markers() {
        assert_eq!(extract_consensus_update("no markers at all"), None);
    }

    // --- extract_reflection / extract_handoff ---

    #[test]
    fn test_extract_reflection() {
        let response = "stuff<<<REFLECTION_START>>>I learned a lot<<<REFLECTION_END>>>more stuff";
        assert_eq!(extract_reflection(response), Some("I learned a lot".to_string()));
    }

    #[test]
    fn test_extract_reflection_missing() {
        assert_eq!(extract_reflection("no reflection here"), None);
    }

    #[test]
    fn test_extract_handoff() {
        let response = "<<<HANDOFF_START>>>Focus on marketing next<<<HANDOFF_END>>>";
        assert_eq!(extract_handoff(response), Some("Focus on marketing next".to_string()));
    }

    // --- extract_skill_requests ---

    #[test]
    fn test_extract_skill_requests_single() {
        let response = "text<<<SKILL_REQUEST>>>deep-research<<<SKILL_REQUEST_END>>>more";
        let result = extract_skill_requests(response);
        assert_eq!(result, vec!["deep-research"]);
    }

    #[test]
    fn test_extract_skill_requests_multiple() {
        let response = "<<<SKILL_REQUEST>>>skill-a<<<SKILL_REQUEST_END>>>middle<<<SKILL_REQUEST>>>skill-b<<<SKILL_REQUEST_END>>>";
        let result = extract_skill_requests(response);
        assert_eq!(result, vec!["skill-a", "skill-b"]);
    }

    #[test]
    fn test_extract_skill_requests_none() {
        assert!(extract_skill_requests("no requests").is_empty());
    }

    #[test]
    fn test_extract_skill_requests_empty_content() {
        let response = "<<<SKILL_REQUEST>>>  <<<SKILL_REQUEST_END>>>";
        assert!(extract_skill_requests(response).is_empty());
    }

    // --- truncate_string ---

    #[test]
    fn test_truncate_within_limit() {
        assert_eq!(truncate_string("short", 100), "short");
    }

    #[test]
    fn test_truncate_exceeds_limit() {
        let result = truncate_string("a long string that exceeds", 10);
        assert_eq!(result, "a long str...");
    }

    #[test]
    fn test_truncate_exact_limit() {
        assert_eq!(truncate_string("exact", 5), "exact");
    }

    // --- build_user_prompt ---

    #[test]
    fn test_build_user_prompt_no_handoff() {
        let result = build_user_prompt("consensus content", "");
        assert!(result.starts_with("Current consensus.md:"));
        assert!(result.contains("consensus content"));
    }

    #[test]
    fn test_build_user_prompt_with_handoff() {
        let result = build_user_prompt("consensus", "handoff note");
        assert!(result.contains("## Handoff from Previous Agent"));
        assert!(result.contains("handoff note"));
        assert!(result.contains("consensus"));
    }
}

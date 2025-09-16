use crate::llm::{Completion, LLMClientDyn};
use eyre::Result;
use rig::message::{AssistantContent, Message};
use serde_json;

/// Extract content from XML tags in a string
pub fn extract_tag(source: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);
    
    let start_pos = source.find(&start_tag)?;
    let content_start = start_pos + start_tag.len();
    let end_pos = source[content_start..].find(&end_tag)?;
    
    Some(source[content_start..content_start + end_pos].trim().to_string())
}

/// Compact an error message using LLM to preserve key information while reducing length
pub async fn compact_error_message(
    llm: &dyn LLMClientDyn,
    model: &str,
    error_msg: &str,
    max_length: usize,
) -> Result<String> {
    if error_msg.len() <= max_length {
        return Ok(error_msg.to_string());
    }

    let original_length = error_msg.len();
    
    let prompt = format!(
        r#"You need to compact an error message to be concise while keeping the most important information.
        The error message is expected be reduced to be less than {} characters approximately.
        Keep the key error type, file paths, line numbers, and the core issue.
        Remove verbose stack traces, repeated information, and non-essential details not helping to understand the root cause.

        Output the compacted error message wrapped in <error> tags.

        The error message to compact is:
        <message>
        {}
        </message>"#,
        max_length, error_msg
    );

    let user_message = Message::user(prompt);

    let completion = Completion::new(model.to_string(), user_message)
        .max_tokens(1024);
    
    match llm.completion(completion).await {
        Ok(response) => {
            if let Some(AssistantContent::Text(text)) = response.choice.iter().next()
                && let Some(compacted) = extract_tag(&text.text, "error") {
                    tracing::info!("Compacted error message from {} to {} characters", original_length, compacted.len());
                    return Ok(compacted);
                }
        }
        Err(e) => {
            tracing::warn!("Failed to compact error message using LLM: {}", e);
        }
    }

    Ok(error_msg.to_string())
}

/// Compact a conversation thread using LLM to reduce token usage while preserving context
pub async fn compact_thread(
    llm: &dyn LLMClientDyn,
    model: &str,
    messages: Vec<Message>,
) -> Result<Vec<Message>> {
    if messages.is_empty() {
        return Ok(messages);
    }

    let last_message = messages.last().unwrap();
    let (thread, residual_messages): (Vec<_>, Vec<_>) = match last_message {
        Message::Assistant { .. } => (messages, Vec::new()),
        Message::User { .. } => {
            let mut msgs = messages;
            let last = msgs.pop().unwrap();
            (msgs, vec![last])
        }
    };

    if thread.is_empty() {
        return Ok(residual_messages);
    }

    // Convert messages to JSON for the prompt
    let thread_json = serde_json::to_string_pretty(&thread)?;
    
    let prompt = format!(
        r#"You need to compact a conversation thread to fit within a token limit.
        Make sure to keep the context and important information, but remove any parts that are not essential for understanding the conversation or outdated.
        Code snippets are not crucial for understanding the conversation, so they can be dropped or replaced with a summary.
        Keep all the details about the user intent, and current status of generation.
        Final output is expected to be ~10 times smaller than the original thread.
        The final output should be structured as two parts: user message and assistant message. Wrap each part in <user> and <assistant> tags respectively.
        
        The conversation thread is as follows:
        {}"#,
        thread_json
    );

    let user_message = Message::user(prompt);

    let completion = Completion::new(model.to_string(), user_message)
        .max_tokens(64 * 1024);

    let response = llm.completion(completion).await?;
    
    if let Some(AssistantContent::Text(text)) = response.choice.iter().next() {
        let mut compacted_messages = Vec::new();
        
        if let Some(user_content) = extract_tag(&text.text, "user") {
            compacted_messages.push(Message::user(user_content));
        }
        
        if let Some(assistant_content) = extract_tag(&text.text, "assistant") {
            compacted_messages.push(Message::assistant(assistant_content));
        }
        
        let residual_count = residual_messages.len();
        compacted_messages.extend(residual_messages);
        
        tracing::info!("Compacted conversation thread from {} to {} messages", thread.len(), compacted_messages.len() - residual_count);
        return Ok(compacted_messages);
    }

    // Fallback: return original messages if compaction failed
    Ok([thread, residual_messages].concat())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tag() {
        let text = "Some text <error>This is the error content</error> more text";
        let result = extract_tag(text, "error");
        assert_eq!(result, Some("This is the error content".to_string()));
    }

    #[test]
    fn test_extract_tag_not_found() {
        let text = "Some text without tags";
        let result = extract_tag(text, "error");
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_tag_with_newlines() {
        let text = r#"<user>
I want to build a web app
with user authentication
</user>"#;
        let result = extract_tag(text, "user");
        assert_eq!(result, Some("I want to build a web app\nwith user authentication".to_string()));
    }
}
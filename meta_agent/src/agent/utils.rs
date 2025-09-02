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

        Example:
        <message>
        tests/test_portfolio_service.py:116:9: F841 Local variable `created_positions` is assigned to but never used
            |
        114 |         ]
        115 |
        116 |         created_positions = [portfolio_service.create_position(data) for data in positions_data]
            |         ^^^^^^^^^^^^^^^^^ F841
        117 |
        118 |         all_positions = portfolio_service.get_all_positions()
            |
            = help: Remove assignment to unused variable `created_positions`

        tests/test_portfolio_service.py:271:9: F841 Local variable `position` is assigned to but never used
            |
        269 |     def test_position_update_validation(self, new_db, portfolio_service, sample_position_data):
        270 |         Test position update validation
        271 |         position = portfolio_service.create_position(sample_position_data)
            |         ^^^^^^^^ F841
        272 |
        273 |         # Test invalid shares update
            |
            = help: Remove assignment to unused variable `position`

        Found 16 errors (14 fixed, 2 remaining).
        No fixes available (2 hidden fixes can be enabled with the `--unsafe-fixes` option).

        Test errors:
        ............................FFF.F.F.....F.F.......                       [100%]
        =================================== FAILURES ===================================
        /app/.venv/lib/python3.12/site-packages/nicegui/testing/user.py:141: AssertionError: expected to see at least one element with marker=Asset Type or content=Asset Type on the page:
        /app/.venv/lib/python3.12/site-packages/nicegui/testing/user.py:217: AssertionError: expected to find at least one element with marker=Save or content=Save on the page:
        /app/.venv/lib/python3.12/site-packages/nicegui/testing/user.py:217: AssertionError: expected to find at least one element with marker=Ticker Symbol or content=Ticker Symbol on the page:
        /app/.venv/lib/python3.12/site-packages/nicegui/testing/user.py:141: AssertionError: expected to see at least one element with marker=STOCK or content=STOCK on the page:
        /app/.venv/lib/python3.12/site-packages/nicegui/testing/user.py:217: AssertionError: expected to find at least one element with marker=Ticker Symbol or content=Ticker Symbol on the page:
        /app/tests/test_price_service.py:65: AssertionError: assert 'BTC:BTC' in 'AssetType.BTC:BTC': Decimal('119121.05'), 'AssetType.ETH:ETH': Decimal('3159.2046'), 'AssetType.STOCK:AAPL': Decimal('209.11')
        /app/tests/test_price_service.py:87: AssertionError: assert 'STOCK:AAPL' in 'AssetType.STOCK:AAPL': Decimal('209.11'), 'AssetType.STOCK:INVALID_TICKER_XYZ': None
        =========================== short test summary info ============================
        FAILED tests/test_portfolio_ui.py::TestPortfolioUI::test_add_position_dialog_opens
        FAILED tests/test_portfolio_ui.py::TestPortfolioUI::test_add_position_form_validation
        FAILED tests/test_portfolio_ui.py::TestPortfolioUI::test_add_position_success
        FAILED tests/test_portfolio_ui.py::TestPortfolioUI::test_portfolio_table_displays_positions
        FAILED tests/test_portfolio_ui.py::TestPortfolioUI::test_portfolio_ui_error_handling
        FAILED tests/test_price_service.py::TestPriceService::test_get_multiple_prices
        FAILED tests/test_price_service.py::TestPriceService::test_get_multiple_prices_with_invalid_ticker
        7 failed, 43 passed, 1 deselected in 6.69s
        </message>


        <error>
        Lint errors:
            tests/test_portfolio_service.py:116:9: F841 Local variable `created_positions` is assigned to but never used
            115 |
            116 |         created_positions = [portfolio_service.create_position(data) for data in positions_data]
                |         ^^^^^^^^^^^^^^^^^ F841
            117 |
            118 |         all_positions = portfolio_service.get_all_positions()

            tests/test_portfolio_service.py:271:9: F841 Local variable `position` is assigned to but never used
            270 |         Test position update validation
            271 |         position = portfolio_service.create_position(sample_position_data)
                |         ^^^^^^^^ F841
            272 |
            273 |         # Test invalid shares update
                |

        Test failures:
            =================================== FAILURES ===================================
            /app/.venv/lib/python3.12/site-packages/nicegui/testing/user.py:141: AssertionError: expected to see at least one element with marker=Asset Type or content=Asset Type on the page:
            /app/.venv/lib/python3.12/site-packages/nicegui/testing/user.py:217: AssertionError: expected to find at least one element with marker=Save or content=Save on the page:
            /app/tests/test_price_service.py:65: AssertionError: assert 'BTC:BTC' in 'AssetType.BTC:BTC': Decimal('119121.05'), 'AssetType.ETH:ETH': Decimal('3159.2046'), 'AssetType.STOCK:AAPL': Decimal('209.11')
            /app/tests/test_price_service.py:87: AssertionError: assert 'STOCK:AAPL' in 'AssetType.STOCK:AAPL': Decimal('119121.05'), 'AssetType.ETH:ETH': Decimal('3159.2046'), 'AssetType.STOCK:AAPL': Decimal('209.11')
            =========================== short test summary info ============================
            FAILED tests/test_portfolio_ui.py::TestPortfolioUI::test_add_position_dialog_opens
            FAILED tests/test_portfolio_ui.py::TestPortfolioUI::test_add_position_form_validation
            FAILED tests/test_portfolio_ui.py::TestPortfolioUI::test_add_position_success
            FAILED tests/test_portfolio_ui.py::TestPortfolioUI::test_portfolio_table_displays_positions
            FAILED tests/test_portfolio_ui.py::TestPortfolioUI::test_portfolio_ui_error_handling
            FAILED tests/test_price_service.py::TestPriceService::test_get_multiple_prices
            FAILED tests/test_price_service.py::TestPriceService::test_get_multiple_prices_with_invalid_ticker
        </error>

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
                    tracing::info!("Compacted error message size: {}, original size: {}", compacted.len(), original_length);
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
        Example:
        <user>
        I want to build a web application that allows users to share photos. It should have user authentication, photo upload, and a feed where users can see photos from others.
        </user>
        <assistant>
        After some work, the application is ready and verified to be working correctly. It includes user authentication, photo upload functionality, and a feed where users can see photos from others.
        I used tools to verify the application and ensure it meets the requirements.
        Feel free to ask for any additional features or improvements!
        </assistant>
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
        
        compacted_messages.extend(residual_messages);
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
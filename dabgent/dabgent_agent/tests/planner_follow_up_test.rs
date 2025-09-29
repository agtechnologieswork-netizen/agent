use dabgent_agent::event::Event;
use dabgent_agent::processor::thread::{Thread, Command};
use dabgent_agent::processor::Aggregate;
use dabgent_agent::llm::{CompletionResponse, FinishReason};

#[test]
fn test_thread_handles_user_input_requested_correctly() {
    // This test verifies the exact sequence that's failing in production
    let mut thread = Thread::new();

    println!("Step 1: Apply LLMConfig");
    thread.apply(&Event::LLMConfig {
        model: "test-model".to_string(),
        temperature: 0.7,
        max_tokens: 4096,
        preamble: Some("You are a planner".to_string()),
        tools: None,
        recipient: Some("planner".to_string()),
        parent: None,
    });

    assert!(thread.model.is_some());
    assert_eq!(thread.recipient, Some("planner".to_string()));

    println!("Step 2: Apply initial UserMessage");
    thread.apply(&Event::UserMessage(
        rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
            text: "create hello world".to_string(),
        }))
    ));

    assert_eq!(thread.messages.len(), 1);
    match &thread.messages[0] {
        rig::completion::Message::User { .. } => {},
        _ => panic!("First message should be User"),
    }

    println!("Step 3: Apply AgentMessage (planner response)");
    thread.apply(&Event::AgentMessage {
        response: CompletionResponse {
            choice: rig::OneOrMany::one(
                rig::message::AssistantContent::Text(rig::message::Text {
                    text: "I'll create a plan".to_string(),
                })
            ),
            finish_reason: FinishReason::Stop,
            output_tokens: 10,
        },
        recipient: Some("planner".to_string()),
    });

    assert_eq!(thread.messages.len(), 2);

    println!("Step 4: Apply tool results (plan creation)");
    // Simulate some tool results
    thread.apply(&Event::ToolResult(vec![])); // Empty for simplicity

    println!("Step 5: Apply PlanCompleted");
    // This doesn't affect the thread messages
    thread.apply(&Event::PlanCompleted {
        tasks: vec!["task1".to_string()],
        message: "Done".to_string(),
    });

    println!("Step 6: Apply UserInputRequested");
    thread.apply(&Event::UserInputRequested {
        prompt: "What would you like to do next?".to_string(),
        context: None,
    });

    println!("Thread messages after UserInputRequested: {}", thread.messages.len());
    for (i, msg) in thread.messages.iter().enumerate() {
        match msg {
            rig::completion::Message::User { .. } => println!("  Message {}: User", i),
            rig::completion::Message::Assistant { .. } => println!("  Message {}: Assistant", i),
        }
    }

    // After UserInputRequested, we should have an Assistant message
    match thread.messages.last() {
        Some(rig::completion::Message::Assistant { .. }) => {
            println!("✓ UserInputRequested added as Assistant message");
        }
        _ => panic!("Last message should be Assistant after UserInputRequested"),
    }

    println!("Step 7: Apply follow-up UserMessage");
    thread.apply(&Event::UserMessage(
        rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
            text: "add emoji".to_string(),
        }))
    ));

    let final_msg_count = thread.messages.len();
    println!("Final message count: {}", final_msg_count);

    // The last message should now be User
    match thread.messages.last() {
        Some(rig::completion::Message::User { .. }) => {
            println!("✓ Follow-up UserMessage added correctly");
        }
        _ => panic!("Last message should be User after follow-up"),
    }

    println!("Step 8: Test if thread can handle agent response");
    // Now test if the thread can process an agent command
    let agent_response = CompletionResponse {
        choice: rig::OneOrMany::one(
            rig::message::AssistantContent::Text(rig::message::Text {
                text: "I'll add emojis".to_string(),
            })
        ),
        finish_reason: FinishReason::Stop,
        output_tokens: 10,
    };

    match thread.handle_agent(Command::Agent(agent_response)) {
        Ok(events) => {
            println!("✓ Thread successfully handled agent response");
            assert_eq!(events.len(), 1);
            match &events[0] {
                Event::AgentMessage { .. } => println!("✓ Generated AgentMessage event"),
                _ => panic!("Should generate AgentMessage event"),
            }
        }
        Err(e) => {
            panic!("Thread failed to handle agent response: {:?}", e);
        }
    }
}

#[test]
fn test_thread_rejects_wrong_message_order() {
    // Test that thread correctly rejects messages in wrong order
    let mut thread = Thread::new();

    // Configure thread
    thread.apply(&Event::LLMConfig {
        model: "test".to_string(),
        temperature: 0.7,
        max_tokens: 100,
        preamble: None,
        tools: None,
        recipient: Some("test".to_string()),
        parent: None,
    });

    // Add a User message
    thread.apply(&Event::UserMessage(
        rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
            text: "hello".to_string(),
        }))
    ));

    // Try to handle another user command (should fail - last message is User)
    let result = thread.handle_user(Command::User(
        rig::OneOrMany::one(rig::message::UserContent::Text(rig::message::Text {
            text: "world".to_string(),
        }))
    ));

    assert!(result.is_err(), "Should reject User after User");
    println!("✓ Correctly rejects User message after User message");
}
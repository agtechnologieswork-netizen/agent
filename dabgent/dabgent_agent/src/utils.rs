//! Common utilities for examples
//! This module provides shared functionality for example programs

use crate::toolbox;
use dabgent_mq::EventStore;
use dabgent_sandbox::SandboxDyn;
use eyre::Result;

// Note: create_memory_store is moved to examples since sqlx is not a direct dependency
// Each example should implement its own store creation

/// Pushes a user prompt to the event store
pub async fn push_prompt<S: EventStore>(
    store: &S,
    stream_id: &str,
    aggregate_id: &str,
    prompt: &str,
) -> Result<()> {
    let user_content = rig::message::UserContent::Text(rig::message::Text { text: prompt.to_owned() });
    let event = crate::event::Event::UserMessage(rig::OneOrMany::one(user_content));
    store
        .push_event(stream_id, aggregate_id, &event, &Default::default())
        .await
        .map_err(Into::into)
}

/// Python validator for running main.py with uv
pub struct PythonValidator;

impl toolbox::Validator for PythonValidator {
    async fn run(&self, sandbox: &mut Box<dyn SandboxDyn>) -> Result<Result<(), String>> {
        sandbox.exec("uv run main.py").await.map(|result| {
            if result.exit_code == 0 {
                Ok(())
            } else {
                Err(format!(
                    "code: {}\nstdout: {}\nstderr: {}",
                    result.exit_code, result.stdout, result.stderr
                ))
            }
        })
    }
}

// Note: create_dagger_sandbox function must be defined in each binary that uses it
// since dagger_sdk is not a direct dependency of dabgent_agent library.
// See the examples for the implementation.
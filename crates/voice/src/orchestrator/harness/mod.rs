//! Harness backend — drives `harness-server` over JSON-RPC on stdio.
//!
//! This is the heaviest provider: it understands the streaming
//! `agent.event` notifications emitted while a conversation is running, so
//! everything from progress reporting to tool-call summaries lives here.
//!
//! Public surface (for `interface.rs`): [`HarnessProvider`].

mod client;
mod protocol;

use crate::orchestrator::interface::{Provider, Session, SendResult};
use crate::orchestrator::progress::ProgressReporter;
use async_trait::async_trait;
use client::HarnessClient;

/// Configured Harness backend. Cheap to clone; spawns one
/// `harness-server` per session.
pub(crate) struct HarnessProvider {
    server_bin: Option<String>,
    initial_conversation_id: Option<String>,
}

impl HarnessProvider {
    pub(crate) fn new(
        server_bin: Option<String>,
        initial_conversation_id: Option<String>,
    ) -> Self {
        Self {
            server_bin,
            initial_conversation_id,
        }
    }

    fn initial_conversation_id_for_slug(&self, slug: &str) -> Option<&str> {
        if matches!(slug, "default" | "main") {
            self.initial_conversation_id.as_deref()
        } else {
            None
        }
    }
}

#[async_trait]
impl Provider for HarnessProvider {
    fn name(&self) -> &'static str {
        "harness"
    }

    async fn open_session(&self, slug: &str) -> Result<Box<dyn Session>, String> {
        let mut client = HarnessClient::spawn(self.server_bin.clone()).await?;
        let conversation_id = if let Some(id) = self.initial_conversation_id_for_slug(slug) {
            id.to_string()
        } else {
            client.create_conversation().await?
        };
        client.set_conversation_id(conversation_id);
        Ok(Box::new(HarnessSession { client }))
    }
}

struct HarnessSession {
    client: HarnessClient,
}

#[async_trait]
impl Session for HarnessSession {
    fn conversation_id(&self) -> &str {
        self.client.conversation_id()
    }

    fn provider_name(&self) -> &'static str {
        "harness"
    }

    async fn send_message_until_done_for_job(
        &mut self,
        job_id: &str,
        message: &str,
        progress: Option<ProgressReporter>,
    ) -> Result<SendResult, String> {
        self.client
            .send_message_until_done_for_job(job_id, message, progress)
            .await
    }
}

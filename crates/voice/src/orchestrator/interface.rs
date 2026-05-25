use super::client::OrchestratorClient;
use super::codex::CodexClient;
use super::protocol::SendResult;

// Provider-facing boundary for background coding agents.
//
// The voice loop should talk in terms of task slugs and messages. The current
// implementation is Harness-over-JSON-RPC, but future providers can be added
// here without changing the Realtime bridge or job manager.
#[derive(Clone, Debug)]
pub(crate) struct OrchestratorProvider {
    kind: OrchestratorProviderKind,
    initial_conversation_id: Option<String>,
}

#[derive(Clone, Debug)]
enum OrchestratorProviderKind {
    Harness { server_bin: Option<String> },
    Codex {
        codex_bin: Option<String>,
        #[allow(dead_code)] // Reserved for future Codex model selection
        model: Option<String>,
    },
}

impl OrchestratorProvider {
    pub(crate) fn harness(
        server_bin: Option<String>,
        initial_conversation_id: Option<String>,
    ) -> Self {
        Self {
            kind: OrchestratorProviderKind::Harness { server_bin },
            initial_conversation_id,
        }
    }

    pub(crate) fn codex(codex_bin: Option<String>, model: Option<String>) -> Self {
        Self {
            kind: OrchestratorProviderKind::Codex {
                codex_bin,
                model,
            },
            initial_conversation_id: None,
        }
    }

    pub(crate) async fn open_session(
        &self,
        slug: &str,
    ) -> Result<OrchestratorSession, String> {
        match &self.kind {
            OrchestratorProviderKind::Harness { server_bin } => {
                let mut client = OrchestratorClient::spawn(server_bin.clone()).await?;
                let conversation_id =
                    if let Some(conversation_id) = self.initial_conversation_id_for_slug(slug)
                    {
                        conversation_id.to_string()
                    } else {
                        client.create_conversation().await?
                    };
                eprintln!(
                    "orchestrator session opened provider=harness slug={} conversation={}",
                    slug, conversation_id
                );
                Ok(OrchestratorSession {
                    slug: slug.to_string(),
                    conversation_id,
                    client: OrchestratorSessionClient::Harness(client),
                })
            }
            OrchestratorProviderKind::Codex {
                codex_bin,
                model,
            } => {
                let client = CodexClient::spawn(codex_bin.clone(), model.clone()).await?;
                let conversation_id = format!("codex-{}", slug);
                eprintln!(
                    "orchestrator session opened provider=codex slug={} conversation={}",
                    slug, conversation_id
                );
                Ok(OrchestratorSession {
                    slug: slug.to_string(),
                    conversation_id,
                    client: OrchestratorSessionClient::Codex(client),
                })
            }
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

enum OrchestratorSessionClient {
    Harness(OrchestratorClient),
    Codex(CodexClient),
}

pub(crate) struct OrchestratorSession {
    slug: String,
    conversation_id: String,
    client: OrchestratorSessionClient,
}

impl OrchestratorSession {
    pub(crate) fn conversation_id(&self) -> &str {
        &self.conversation_id
    }

    pub(crate) async fn send_message_until_done_for_job(
        &mut self,
        job_id: &str,
        message: &str,
    ) -> Result<SendResult, String> {
        eprintln!(
            "orchestrator session send provider={} slug={} job={} conversation={} message_bytes={}",
            self.provider_name(),
            self.slug,
            job_id,
            self.conversation_id,
            message.len()
        );
        match &mut self.client {
            OrchestratorSessionClient::Harness(client) => {
                client
                    .send_message_until_done_for_job(job_id, &self.conversation_id, message)
                    .await
            }
            OrchestratorSessionClient::Codex(client) => {
                client
                    .send_message_until_done_for_job(job_id, &self.conversation_id, message)
                    .await
            }
        }
    }

    fn provider_name(&self) -> &'static str {
        match self.client {
            OrchestratorSessionClient::Harness(_) => "harness",
            OrchestratorSessionClient::Codex(_) => "codex",
        }
    }
}

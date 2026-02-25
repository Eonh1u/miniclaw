//! LLM Client module.
//!
//! This module defines the `LlmProvider` trait that abstracts over different
//! LLM API providers (Anthropic, OpenAI, etc.), and provides concrete
//! implementations.
//!
//! Key concepts:
//! - **Trait**: Rust's way of defining shared behavior (like interfaces)
//! - **async_trait**: since Rust traits don't natively support async fn,
//!   we use the async-trait crate to enable async methods in traits
//! - **Provider pattern**: each LLM API has its own request/response format,
//!   but they all implement the same trait so the rest of the code doesn't care

pub mod anthropic;
pub mod openai_compatible;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::types::{ChatRequest, ChatResponse, StreamChunk};

/// Trait that all LLM providers must implement.
///
/// This is the core abstraction that allows swapping between
/// Anthropic, OpenAI, or any other provider without changing
/// the agent logic.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send a chat completion request and get a full response.
    ///
    /// This is the non-streaming version: it waits for the entire
    /// response before returning.
    async fn chat_completion(&self, request: &ChatRequest) -> Result<ChatResponse>;

    /// Send a streaming chat completion request.
    ///
    /// Text deltas are sent via `chunk_tx` in real-time. The method
    /// returns the fully accumulated `ChatResponse` when the stream ends.
    /// Default implementation falls back to non-streaming.
    async fn chat_completion_stream(
        &self,
        request: &ChatRequest,
        chunk_tx: mpsc::UnboundedSender<StreamChunk>,
    ) -> Result<ChatResponse> {
        let response = self.chat_completion(request).await?;
        if !response.content.is_empty() {
            let _ = chunk_tx.send(StreamChunk::TextDelta(response.content.clone()));
        }
        let _ = chunk_tx.send(StreamChunk::Done);
        Ok(response)
    }

    /// Return the provider's display name (for logging).
    fn name(&self) -> &str;
}

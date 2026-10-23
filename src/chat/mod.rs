//! ACP chat: one long-lived agent process per chat id, streaming tabsh-shaped chunks.

pub mod acp;
pub mod chunk;
pub mod manager;
pub mod models;
pub mod normalize;
pub mod transcript;

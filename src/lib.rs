use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

pub type Tx = mpsc::UnboundedSender<String>;
pub type Rx = mpsc::UnboundedReceiver<String>;

#[derive(Debug)]
pub struct ChatState {
    // username -> sender to that user
    peers: HashMap<String, Tx>,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    pub fn join(&mut self, username: String, tx: Tx) -> Result<(), JoinError> {
        if self.peers.contains_key(&username) {
            return Err(JoinError::UsernameTaken(username));
        }
        self.peers.insert(username, tx);
        Ok(())
    }

    pub fn leave(&mut self, username: &str) {
        self.peers.remove(username);
    }

    pub fn broadcast(&self, from: &str, msg: &str) {
        let payload = format!("FROM {} {}\n", from, msg);
        for (user, tx) in &self.peers {
            if user == from {
                continue;
            }
            let _ = tx.send(payload.clone());
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum JoinError {
    #[error("username `{0}` is already taken")]
    UsernameTaken(String),
}

pub type SharedState = Arc<RwLock<ChatState>>;

/// Simple line-based protocol:
/// Client -> Server:
///   JOIN <username>\n
///   MSG <text>\n
///   LEAVE\n
///
/// Server -> Client:
///   INFO <text>\n
///   ERR <text>\n
///   FROM <username> <text>\n
pub mod protocol {
    pub const CMD_JOIN: &str = "JOIN";
    pub const CMD_MSG: &str = "MSG";
    pub const CMD_LEAVE: &str = "LEAVE";

    pub fn info(msg: &str) -> String {
        format!("INFO {}\n", msg)
    }

    pub fn err(msg: &str) -> String {
        format!("ERR {}\n", msg)
    }
}

use anyhow::Result;
use async_chat::{protocol, ChatState, JoinError, SharedState, Tx};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};

#[tokio::main]
async fn main() -> Result<()> {
    let addr = std::env::var("CHAT_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = TcpListener::bind(&addr).await?;
    println!("Server listening on {addr}");

    let state: SharedState = Arc::new(RwLock::new(ChatState::new()));

    loop {
        let (socket, peer_addr) = listener.accept().await?;
        let state = state.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, state, peer_addr).await {
                eprintln!("connection error from {peer_addr}: {e:?}");
            }
        });
    }
}

async fn handle_connection(
    socket: TcpStream,
    state: SharedState,
    peer_addr: SocketAddr,
) -> Result<()> {
    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // 1. Expect JOIN <username>
    line.clear();
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
        return Ok(());
    }
    let mut parts = line.trim_end().splitn(2, ' ');
    let cmd = parts.next().unwrap_or_default();
    if cmd != protocol::CMD_JOIN {
        writer
            .write_all(protocol::err("first command must be JOIN <username>").as_bytes())
            .await?;
        return Ok(());
    }
    let username = match parts.next() {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => {
            writer
                .write_all(protocol::err("username is required").as_bytes())
                .await?;
            return Ok(());
        }
    };

    // Create a channel for server -> this client
    let (tx, mut rx): (Tx, _) = mpsc::unbounded_channel();

    // Try to join
    {
        let mut guard = state.write().await;
        if let Err(e) = guard.join(username.clone(), tx) {
            match e {
                JoinError::UsernameTaken(_) => {
                    writer
                        .write_all(protocol::err("username taken").as_bytes())
                        .await?;
                    return Ok(());
                }
            }
        }
    }

    writer
        .write_all(protocol::info("joined chat").as_bytes())
        .await?;

    // Task to forward messages from channel to socket
    let mut writer_clone = writer.clone();
    let username_clone = username.clone();
    let forward_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if writer_clone.write_all(msg.as_bytes()).await.is_err() {
                eprintln!("write error to {username_clone}");
                break;
            }
        }
    });

    // 2. Handle incoming commands
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            // client disconnected
            break;
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.splitn(2, ' ');
        let cmd = parts.next().unwrap_or_default();

        match cmd {
            cmd if cmd == protocol::CMD_MSG => {
                let msg = parts.next().unwrap_or("");
                let guard = state.read().await;
                guard.broadcast(&username, msg);
            }
            cmd if cmd == protocol::CMD_LEAVE => {
                break;
            }
            _ => {
                writer
                    .write_all(protocol::err("unknown command").as_bytes())
                    .await?;
            }
        }
    }

    // Cleanup
    {
        let mut guard = state.write().await;
        guard.leave(&username);
    }

    forward_task.abort(); // stop the forwarder; channel will be dropped anyway
    println!("user `{username}` disconnected ({peer_addr})");

    Ok(())
}

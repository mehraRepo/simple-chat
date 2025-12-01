use anyhow::Result;
use clap::Parser;
use std::io;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

#[derive(Parser, Debug)]
struct Args {
    /// Server host
    #[arg(long, env = "CHAT_HOST", default_value = "127.0.0.1")]
    host: String,

    /// Server port
    #[arg(long, env = "CHAT_PORT", default_value = "8080")]
    port: u16,

    /// Username
    #[arg(long, env = "CHAT_USER")]
    username: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let addr = format!("{}:{}", args.host, args.port);
    println!("Connecting to {addr} as `{}`...", args.username);
    let stream = TcpStream::connect(addr).await?;
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // Send JOIN
    writer
        .write_all(format!("JOIN {}\n", args.username).as_bytes())
        .await?;

    // Read initial response (INFO or ERR)
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    if line.starts_with("ERR ") {
        eprintln!("Server error: {}", line.trim_end());
        return Ok(());
    }
    println!("{}", line.trim_end());

    // Task to read from server and print
    let mut reader_for_task = reader;
    let recv_task = tokio::spawn(async move {
        let mut buf = String::new();
        loop {
            buf.clear();
            match reader_for_task.read_line(&mut buf).await {
                Ok(0) => {
                    println!("Disconnected from server.");
                    break;
                }
                Ok(_) => {
                    print!("\r{}\n> ", buf.trim_end());
                    let _ = io::Write::flush(&mut io::stdout());
                }
                Err(e) => {
                    eprintln!("Error reading from server: {e}");
                    break;
                }
            }
        }
    });

    // Main interactive prompt
    println!("Commands:\n  send <MSG>\n  leave");
    print!("> ");
    io::Write::flush(&mut io::stdout())?;

    let mut stdin_reader = BufReader::new(tokio::io::stdin());
    let mut input = String::new();

    loop {
        input.clear();
        let n = stdin_reader.read_line(&mut input).await?;
        if n == 0 {
            // EOF from stdin
            break;
        }
        let trimmed = input.trim_end();
        if trimmed.is_empty() {
            print!("> ");
            io::Write::flush(&mut io::stdout())?;
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("send ") {
            writer
                .write_all(format!("MSG {}\n", rest).as_bytes())
                .await?;
        } else if trimmed == "leave" {
            writer.write_all(b"LEAVE\n").await?;
            break;
        } else {
            println!("Unknown command. Use `send <MSG>` or `leave`.");
        }
        print!("> ");
        io::Write::flush(&mut io::stdout())?;
    }

    recv_task.abort();
    Ok(())
}

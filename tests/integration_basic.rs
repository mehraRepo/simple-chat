use assert_cmd::Command;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn server_and_client_start() {
    // Start server
    let mut server = Command::cargo_bin("server").unwrap();
    server.env("CHAT_ADDR", "127.0.0.1:9001");
    let mut server_child = server.spawn().unwrap();

    // Give server a moment
    sleep(Duration::from_millis(300)).await;

    // Start client that joins and then leaves
    let mut client = Command::cargo_bin("client").unwrap();
    client
        .env("CHAT_HOST", "127.0.0.1")
        .env("CHAT_PORT", "9001")
        .env("CHAT_USER", "tester")
        .write_stdin("leave\n");

    let assert = client.assert();
    assert.success();

    // Kill server
    let _ = server_child.kill();
}

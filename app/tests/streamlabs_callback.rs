//! OAuth loopback callback-server tests.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use tiktok_stream_tool::domain::errors::AuthError;
use tiktok_stream_tool::integrations::streamlabs::auth;

async fn send(port: u16, target: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let request = format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response
}

#[tokio::test(flavor = "multi_thread")]
async fn valid_callback_returns_code() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = "expected-state".to_string();

    let server = tokio::spawn({
        let state = state.clone();
        async move { auth::listen_for_callback(listener, &state, Duration::from_secs(5)).await }
    });

    let response = send(
        port,
        "/?success=true&code=the-auth-code&state=expected-state",
    )
    .await;
    assert!(response.starts_with("HTTP/1.1 200"), "response: {response}");

    let code = server.await.unwrap().unwrap();
    assert_eq!(code, "the-auth-code");
}

#[tokio::test(flavor = "multi_thread")]
async fn callback_without_state_is_accepted() {
    // Streamlabs' Python flow never echoes `state`; a callback with no state
    // must still succeed (PKCE protects the exchange).
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = "expected-state".to_string();

    let server = tokio::spawn({
        let state = state.clone();
        async move { auth::listen_for_callback(listener, &state, Duration::from_secs(5)).await }
    });

    let response = send(port, "/?success=true&code=no-state-code").await;
    assert!(response.starts_with("HTTP/1.1 200"), "response: {response}");

    let code = server.await.unwrap().unwrap();
    assert_eq!(code, "no-state-code");
}

#[tokio::test(flavor = "multi_thread")]
async fn state_mismatch_does_not_block_callback() {
    // Streamlabs may echo its own `state`; PKCE binds the code, so a callback
    // with a non-matching state must still succeed.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = "expected-state".to_string();

    let server = tokio::spawn({
        let state = state.clone();
        async move { auth::listen_for_callback(listener, &state, Duration::from_secs(5)).await }
    });

    let response = send(port, "/?success=true&code=abc&state=attacker").await;
    assert!(response.starts_with("HTTP/1.1 200"), "response: {response}");

    assert_eq!(server.await.unwrap().unwrap(), "abc");
}

#[tokio::test(flavor = "multi_thread")]
async fn callback_without_success_is_accepted() {
    // The internal redirect does not reliably echo `success`.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = "expected-state".to_string();

    let server = tokio::spawn({
        let state = state.clone();
        async move { auth::listen_for_callback(listener, &state, Duration::from_secs(5)).await }
    });

    let response = send(port, "/?code=no-success-code").await;
    assert!(response.starts_with("HTTP/1.1 200"), "response: {response}");

    assert_eq!(server.await.unwrap().unwrap(), "no-success-code");
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_denial_is_rejected() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = "expected-state".to_string();

    let server = tokio::spawn({
        let state = state.clone();
        async move { auth::listen_for_callback(listener, &state, Duration::from_secs(5)).await }
    });

    let response = send(port, "/?success=false").await;
    assert!(response.starts_with("HTTP/1.1 400"), "response: {response}");

    assert!(matches!(server.await.unwrap(), Err(AuthError::Denied)));
}

#[tokio::test(flavor = "multi_thread")]
async fn ignores_unknown_paths_then_accepts_callback() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = "expected-state".to_string();

    let server = tokio::spawn({
        let state = state.clone();
        async move { auth::listen_for_callback(listener, &state, Duration::from_secs(5)).await }
    });

    let not_found = send(port, "/favicon.ico").await;
    assert!(
        not_found.starts_with("HTTP/1.1 404"),
        "response: {not_found}"
    );

    let ok = send(port, "/?success=true&code=xyz&state=expected-state").await;
    assert!(ok.starts_with("HTTP/1.1 200"), "response: {ok}");

    assert_eq!(server.await.unwrap().unwrap(), "xyz");
}

#[tokio::test(flavor = "multi_thread")]
async fn times_out_without_a_callback() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let state = "expected-state".to_string();
    let result = auth::listen_for_callback(listener, &state, Duration::from_millis(200)).await;
    assert!(matches!(result, Err(AuthError::Timeout)));
}

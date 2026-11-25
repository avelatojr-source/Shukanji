//! Streamlabs client tests using a mock HTTP server.

use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use tiktok_stream_tool::domain::errors::StreamlabsError;
use tiktok_stream_tool::domain::models::{AudienceType, SecretString};
use tiktok_stream_tool::integrations::streamlabs::{
    MAX_CATEGORY_LEN, StartStreamResult, StreamlabsClient,
};

fn client(server: &MockServer) -> StreamlabsClient {
    StreamlabsClient::with_base_url(SecretString::new("test-token"), server.uri()).unwrap()
}

#[tokio::test]
async fn search_games_returns_categories_and_other() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/tiktok/info"))
        .and(query_param("category", "VALORANT"))
        .and(header("authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "categories": [
                {"full_name": "VALORANT", "game_mask_id": "12345"}
            ]
        })))
        .mount(&server)
        .await;

    let categories = client(&server).search_games("VALORANT").await.unwrap();
    assert_eq!(categories.len(), 2);
    assert_eq!(categories[0].full_name, "VALORANT");
    assert_eq!(categories[0].game_mask_id, "12345");
    assert_eq!(categories[1].full_name, "Other");
}

#[tokio::test]
async fn search_games_truncates_long_names() {
    let server = MockServer::start().await;
    let long = "x".repeat(40);
    let truncated: String = long.chars().take(MAX_CATEGORY_LEN).collect();
    Mock::given(method("GET"))
        .and(path("/tiktok/info"))
        .and(query_param("category", truncated.as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"categories": []})))
        .mount(&server)
        .await;

    let categories = client(&server).search_games(&long).await.unwrap();
    assert_eq!(categories.len(), 1); // only the appended "Other"
}

#[tokio::test]
async fn search_games_empty_query_makes_no_request() {
    let server = MockServer::start().await;
    // No mock is mounted; wiremock would return 404 if a request were made.
    let categories = client(&server).search_games("   ").await.unwrap();
    assert!(categories.is_empty());
}

#[tokio::test]
async fn account_info_parses_user_and_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/tiktok/info"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user": {"username": "creator_one"},
            "application_status": {"status": "approved"},
            "can_be_live": true
        })))
        .mount(&server)
        .await;

    let info = client(&server).account_info().await.unwrap();
    assert_eq!(info.username, "creator_one");
    assert_eq!(info.application_status, "approved");
    assert!(info.can_be_live);
}

#[tokio::test]
async fn start_stream_returns_credentials() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tiktok/stream/start"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "stream-1",
            "rtmp": "rtmp://live.tiktok.com/live/",
            "key": "the-stream-key"
        })))
        .mount(&server)
        .await;

    let result = client(&server)
        .start_stream("Title", "12345", AudienceType::General)
        .await
        .unwrap();
    match result {
        StartStreamResult::Started {
            stream_id,
            server,
            key,
        } => {
            assert_eq!(stream_id, "stream-1");
            assert_eq!(server, "rtmp://live.tiktok.com/live/");
            assert_eq!(key.expose(), "the-stream-key");
        }
        other => panic!("expected Started, got {other:?}"),
    }
}

#[tokio::test]
async fn start_stream_server_error_is_outcome_unknown() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tiktok/stream/start"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let result = client(&server)
        .start_stream("Title", "", AudienceType::General)
        .await
        .unwrap();
    assert!(matches!(result, StartStreamResult::OutcomeUnknown { .. }));
}

#[tokio::test]
async fn start_stream_missing_fields_is_no_credentials() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tiktok/stream/start"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"error": "nope"})))
        .mount(&server)
        .await;

    let err = client(&server)
        .start_stream("Title", "", AudienceType::General)
        .await
        .unwrap_err();
    assert!(matches!(err, StreamlabsError::NoCredentials));
}

#[tokio::test]
async fn end_stream_reports_success() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tiktok/stream/stream-1/end"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success": true})))
        .mount(&server)
        .await;

    assert!(client(&server).end_stream("stream-1").await.unwrap());
}

#[tokio::test]
async fn http_error_is_surfaced() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/tiktok/info"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let err = client(&server).account_info().await.unwrap_err();
    assert!(matches!(err, StreamlabsError::Http { status: 403 }));
}

#[tokio::test]
async fn subjecting_a_client_debug_does_not_leak_token() {
    let server = MockServer::start().await;
    let client =
        StreamlabsClient::with_base_url(SecretString::new("very-secret"), server.uri()).unwrap();
    let rendered = format!("{client:?}");
    assert!(!rendered.contains("very-secret"));
}

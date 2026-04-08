//! End-to-end **gap_fill** session: Docker Postgres, migrations + seed, full HTTP API playthrough
//! (draft → start materializes passage + one multi-gap step → one answer → result).

use crate::common::http::{assert_status, json_roundtrip, parse_json};
use axum::http::StatusCode;
use axum::Router;
use serde_json::json;
use shakti_game_domain::GameResult;
use shakti_game_engine::support::{
    build_app_router, build_app_state, connect_pool, run_migrations,
};
use shakti_game_infrastructure::{build_llm_preparer, LlmMode};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

const USER_ID: &str = "11111111-1111-1111-1111-111111111111";

/// Order matches `start_char` in mock-built passage (see seeded `learning_items` + `user_hard_words`).
const EXPECTED_GAPS: &[&str] = &["gikk", "svart", "regner", "leser", "hjem"];

#[tokio::test(flavor = "multi_thread")]
async fn gap_fill_full_session_lifecycle() {
    let _ = tracing_subscriber::fmt::try_init();

    let container = Postgres::default()
        .start()
        .await
        .expect("start postgres container");
    let host = container.get_host().await.expect("container host");
    let port = container.get_host_port_ipv4(5432).await.expect("host port");
    let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");

    let pool = connect_pool(&url).await.expect("connect pool");
    run_migrations(&pool).await.expect("migrations");

    let llm = build_llm_preparer(
        LlmMode::Mock,
        None,
        "gpt-4o-mini".into(),
        None,
        "openai_main".into(),
        "shakti-game-engine".into(),
    )
    .expect("llm preparer");
    let app: Router = build_app_router(build_app_state(pool, llm, false, None));
    let user = Uuid::parse_str(USER_ID).unwrap();

    let (st, body) = json_roundtrip(
        app.clone(),
        "POST",
        "/api/v1/game-sessions",
        Some(json!({
            "userId": user,
            "gameKind": "gap_fill",
            "contentRequest": { "source": "hard_words", "limit": 10, "language": "no" },
        })),
    )
    .await;
    assert_status(st, StatusCode::OK, &body);
    let session_id: Uuid = serde_json::from_value(body["sessionId"].clone()).unwrap();
    assert_eq!(body["stepsCount"], 0);
    assert_eq!(body["state"], "draft");

    let (st, body) = json_roundtrip(
        app.clone(),
        "POST",
        &format!("/api/v1/game-sessions/{session_id}/start"),
        Some(json!({ "userId": user })),
    )
    .await;
    assert_status(st, StatusCode::OK, &body);
    assert_eq!(body["state"], "in_progress");
    assert_eq!(body["stepsCount"], 1);

    let (st, view) = json_roundtrip(
        app.clone(),
        "GET",
        &format!("/api/v1/game-sessions/{session_id}?userId={user}"),
        None,
    )
    .await;
    assert_status(st, StatusCode::OK, &view);
    let step_id: Uuid =
        serde_json::from_value(view["currentStep"]["id"].clone()).expect("step id");

    let selections: Vec<&str> = EXPECTED_GAPS.to_vec();
    let (st, ans) = json_roundtrip(
        app.clone(),
        "POST",
        &format!("/api/v1/game-sessions/{session_id}/steps/{step_id}/answer"),
        Some(json!({
            "userId": user,
            "answer": { "type": "gap_fill_slots", "selections": selections },
        })),
    )
    .await;
    assert_status(st, StatusCode::OK, &ans);
    assert!(ans["correct"].as_bool().unwrap());
    assert_eq!(ans["sessionState"], "completed");

    let (st, res_body) = json_roundtrip(
        app.clone(),
        "GET",
        &format!("/api/v1/game-sessions/{session_id}/result?userId={user}"),
        None,
    )
    .await;
    assert_status(st, StatusCode::OK, &res_body);
    let result: GameResult = parse_json(&res_body);
    assert_eq!(result.score.earned_points, 50);
    assert_eq!(result.score.total_points, 50);
}

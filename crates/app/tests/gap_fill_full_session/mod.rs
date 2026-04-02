//! End-to-end **gap_fill** session: Docker Postgres, migrations + seed, full HTTP API playthrough.
//!
//! ## Template for another game
//! 1. Add a sibling directory e.g. `tests/morph_gap_fill/` with `mod.rs` implementing your scenario.
//! 2. Add `mod morph_gap_fill;` in `integration_test.rs`.
//! 3. Reuse `crate::common::http` for JSON round-trips.
//!
//! Requires Docker. Run: `cargo test -p shakti-game-engine --test integration_test`

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

/// Matches seeded `learning_items` order (`ORDER BY created_at ASC, id ASC`).
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

    let llm = build_llm_preparer(LlmMode::Mock, None, "gpt-4o-mini".into()).expect("llm preparer");
    let app: Router = build_app_router(build_app_state(pool, llm));
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
    assert_eq!(body["stepsCount"], 5);
    assert_eq!(body["state"], "prepared");

    let (st, body) = json_roundtrip(
        app.clone(),
        "POST",
        &format!("/api/v1/game-sessions/{session_id}/start"),
        Some(json!({ "userId": user })),
    )
    .await;
    assert_status(st, StatusCode::OK, &body);
    assert_eq!(body["state"], "in_progress");

    for (i, word) in EXPECTED_GAPS.iter().enumerate() {
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

        let (st, ans) = json_roundtrip(
            app.clone(),
            "POST",
            &format!("/api/v1/game-sessions/{session_id}/steps/{step_id}/answer"),
            Some(json!({
                "userId": user,
                "answer": { "type": "text", "value": word },
            })),
        )
        .await;
        assert_status(st, StatusCode::OK, &ans);
        assert!(
            ans["correct"].as_bool().unwrap(),
            "step {i} should be correct"
        );

        let last = i + 1 == EXPECTED_GAPS.len();
        if !last {
            assert_eq!(ans["sessionState"], "in_progress");
            let (st, adv) = json_roundtrip(
                app.clone(),
                "POST",
                &format!("/api/v1/game-sessions/{session_id}/advance"),
                Some(json!({ "userId": user })),
            )
            .await;
            assert_status(st, StatusCode::OK, &adv);
        } else {
            assert_eq!(ans["sessionState"], "completed");
        }
    }

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

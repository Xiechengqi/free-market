mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use dujiao_rust::web::router::router;
use tower::ServiceExt;

#[tokio::test]
async fn check_geetest_route_removed() {
    let env = common::boot().await;
    let app = router(env.state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/check-geetest")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

mod common;

use dujiao_rust::services::{order_service, payment_service};

/// Under heavy contention the invariant is "no oversell": total reserved cards must
/// equal the number of successful orders, and no card may belong to more than one
/// order. Some buyers can legitimately lose the race and get `Conflict` back — that's
/// expected SQLite-single-writer behavior and the client is responsible for retrying.
#[tokio::test]
async fn concurrent_buyers_never_oversell() {
    let env = common::boot().await;
    let state = env.state.clone();
    let mut handles = Vec::new();
    for i in 0..10 {
        let state = state.clone();
        handles.push(tokio::spawn(async move {
            let email = format!("buyer{i}@example.com");
            let ip = format!("10.0.0.{i}");
            let form = order_service::CreateOrderForm {
                gid: 1,
                email,
                by_amount: 1,
                payway: 1,
                search_pwd: None,
                coupon_code: None,
                captcha_id: None,
                captcha_answer: None,
                extra: std::collections::HashMap::new(),
            };
            order_service::create_guest_order(&state, form, ip).await
        }));
    }
    let mut ok = 0;
    for h in handles {
        if h.await.unwrap().is_ok() {
            ok += 1;
        }
    }
    let reserved = common::count_card_status(&state.pool, "reserved").await;
    let available = common::count_card_status(&state.pool, "available").await;
    assert_eq!(
        reserved, ok,
        "reserved cards must equal number of successful orders"
    );
    assert!(ok <= 5, "no more than 5 cards exist so at most 5 successes");
    assert_eq!(
        reserved + available,
        5,
        "no card may be lost during contention"
    );
    let duplicate_owners: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM (SELECT order_id FROM card_secrets WHERE order_id IS NOT NULL GROUP BY order_id HAVING COUNT(*) > 1)",
    )
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(duplicate_owners, 0, "no order may own more than one card");
}

/// Payment callback with mismatched amount must be rejected.
#[tokio::test]
async fn payment_amount_mismatch_rejected() {
    let env = common::boot().await;
    let state = env.state.clone();
    let order_no = common::make_order(&state, "x@example.com", "10.0.0.99").await;
    let payment_id = common::create_payment_for(&state, &order_no).await;

    let result = payment_service::apply_success(&state, payment_id, 12345, "CNY").await;
    assert!(result.is_err(), "amount mismatch must error");
    let status: String = sqlx::query_scalar("SELECT status FROM orders WHERE order_no = ?")
        .bind(&order_no)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(status, "pending_payment");
}

/// Repeated successful callbacks for the same payment must be idempotent.
#[tokio::test]
async fn repeated_success_callback_idempotent() {
    let env = common::boot().await;
    let state = env.state.clone();
    let order_no = common::make_order(&state, "y@example.com", "10.0.0.98").await;
    let payment_id = common::create_payment_for(&state, &order_no).await;

    payment_service::apply_success(&state, payment_id, 1000, "CNY")
        .await
        .expect("first callback");
    // Re-applying must not change status, must not double-fulfill, must not error.
    payment_service::apply_success(&state, payment_id, 1000, "CNY")
        .await
        .expect("second callback idempotent");

    let order_status: String = sqlx::query_scalar("SELECT status FROM orders WHERE order_no = ?")
        .bind(&order_no)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(order_status, "completed");
    let fulfillments: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM fulfillments WHERE order_id = (SELECT id FROM orders WHERE order_no = ?)")
            .bind(&order_no)
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(fulfillments, 1, "exactly one fulfillment record");
}

/// When a coupon-bearing order is canceled twice, the coupon's `used_count` only goes back by 1.
#[tokio::test]
async fn coupon_refund_is_idempotent() {
    let env = common::boot().await;
    let state = env.state.clone();
    let form = order_service::CreateOrderForm {
        gid: 1,
        email: "coupon@example.com".to_string(),
        by_amount: 1,
        payway: 1,
        search_pwd: None,
        coupon_code: Some("TENOFF".to_string()),
        captcha_id: None,
        captcha_answer: None,
        extra: std::collections::HashMap::new(),
    };
    let order_no = order_service::create_guest_order(&state, form, "10.0.0.50".to_string())
        .await
        .unwrap();

    let used_after_create: i64 = sqlx::query_scalar("SELECT used_count FROM coupons WHERE id = 1")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(used_after_create, 1);

    let order_id: i64 = sqlx::query_scalar("SELECT id FROM orders WHERE order_no = ?")
        .bind(&order_no)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    order_service::cancel_expired_order(&state, order_id)
        .await
        .unwrap();
    // Second cancel should be a no-op for refund accounting.
    order_service::cancel_expired_order(&state, order_id)
        .await
        .unwrap();

    let used_after_cancel: i64 = sqlx::query_scalar("SELECT used_count FROM coupons WHERE id = 1")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        used_after_cancel, 0,
        "coupon used_count must decrement exactly once"
    );

    let ret_back_flag: i64 = sqlx::query_scalar("SELECT coupon_ret_back FROM orders WHERE id = ?")
        .bind(order_id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(ret_back_flag, 1);
}

/// Canceling an unpaid order releases its reserved card secret back to `available`.
#[tokio::test]
async fn cancel_releases_reserved_cards() {
    let env = common::boot().await;
    let state = env.state.clone();
    let order_no = common::make_order(&state, "z@example.com", "10.0.0.40").await;

    assert_eq!(common::count_card_status(&state.pool, "available").await, 4);
    assert_eq!(common::count_card_status(&state.pool, "reserved").await, 1);

    let order_id: i64 = sqlx::query_scalar("SELECT id FROM orders WHERE order_no = ?")
        .bind(&order_no)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    order_service::cancel_expired_order(&state, order_id)
        .await
        .unwrap();

    assert_eq!(common::count_card_status(&state.pool, "available").await, 5);
    assert_eq!(common::count_card_status(&state.pool, "reserved").await, 0);
}

/// Loop cards limit buy-amount to 1 when only loop cards are available.
#[tokio::test]
async fn loop_card_limits_purchase_to_one() {
    let env = common::boot().await;
    let state = env.state.clone();
    // Remove the 5 normal cards, leave only a single loop card.
    sqlx::query("DELETE FROM card_secrets WHERE product_id = 1")
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO card_secrets(product_id, sku_id, secret, status, is_loop, created_at, updated_at)
         VALUES (1, 0, 'LOOP', 'available', 1, ?, ?)",
    )
    .bind("2026-06-16T00:00:00+00:00")
    .bind("2026-06-16T00:00:00+00:00")
    .execute(&state.pool)
    .await
    .unwrap();

    // Buying 2 must fail.
    let form = order_service::CreateOrderForm {
        gid: 1,
        email: "loop@example.com".to_string(),
        by_amount: 2,
        payway: 1,
        search_pwd: None,
        coupon_code: None,
        captcha_id: None,
        captcha_answer: None,
        extra: std::collections::HashMap::new(),
    };
    let err = order_service::create_guest_order(&state, form, "10.0.0.7".to_string()).await;
    assert!(err.is_err(), "loop card must reject by_amount > 1");

    // Buying 1 must succeed and the loop card stays referenced.
    let form_ok = order_service::CreateOrderForm {
        gid: 1,
        email: "loop@example.com".to_string(),
        by_amount: 1,
        payway: 1,
        search_pwd: None,
        coupon_code: None,
        captcha_id: None,
        captcha_answer: None,
        extra: std::collections::HashMap::new(),
    };
    let order_no = order_service::create_guest_order(&state, form_ok, "10.0.0.7".to_string())
        .await
        .expect("buy 1 loop card");
    assert!(!order_no.is_empty());
}

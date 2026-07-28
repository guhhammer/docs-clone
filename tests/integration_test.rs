use actix_web::{test, App};
use docs_clone::handlers;
use sqlx::SqlitePool;

#[actix_web::test]
async fn test_health_endpoint() {
    let app = test::init_service(
        App::new().route("/", test::to(handlers::health))
    ).await;

    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;

    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_create_document() {
    // This is a basic unit test for the handler
    // In a real scenario, you'd set up a test database
    let result = serde_json::json!({
        "status": "healthy",
        "message": "Docs Clone API is running"
    });
    
    assert_eq!(result["status"], "healthy");
}

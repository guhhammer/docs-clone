//! Application entrypoint and server composition root.
//!
//! The whole product (API + server-rendered UI + static assets) is served from a
//! single Actix instance on `compile_config::SERVER_BIND_PORT` (17777).

mod compile_config;
mod db;
mod handlers;
mod models;
mod security;

use actix_files as fs;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{
    middleware::{Compress, DefaultHeaders, NormalizePath},
    web, App, HttpServer,
};
use std::env;
use tera::Tera;
use tokio::time::Duration;

use compile_config::*;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let mongo_url = env::var("MONGODB_URL").unwrap_or_else(|_| DEFAULT_MONGODB_URL.to_string());

    let db = db::Db::new(&mongo_url)
        .await
        .expect("Failed to connect to MongoDB");

    let tera = Tera::new(TEMPLATES_GLOB).expect("Failed to load templates");

    // Bind host/port are compile-time constants; `BIND_ADDRESS` may override them
    // for containerized deployments only.
    let bind_address = env::var("BIND_ADDRESS")
        .unwrap_or_else(|_| format!("{}:{}", SERVER_BIND_HOST, SERVER_BIND_PORT));

    println!("Server starting at http://{}", bind_address);

    HttpServer::new(move || {
        let governor_conf = GovernorConfigBuilder::default()
            .seconds_per_request(RATE_LIMIT_SECONDS_PER_REQUEST)
            .burst_size(RATE_LIMIT_BURST_SIZE)
            .finish()
            .expect("Invalid rate limit configuration");

        App::new()
            .app_data(web::Data::new(db.clone()))
            .app_data(web::Data::new(tera.clone()))
            .app_data(web::JsonConfig::default().limit(MAX_JSON_PAYLOAD_BYTES))
            .wrap(
                DefaultHeaders::new()
                    .add(HEADER_CSP)
                    .add(HEADER_PERMISSIONS_POLICY)
                    .add(HEADER_XXS_PROTECTION)
                    .add(HEADER_X_FRAME_OPTIONS)
                    .add(HEADER_X_CONTENT_TYPE_OPTIONS)
                    .add(HEADER_REFERRER_POLICY)
                    .add(HEADER_HSTS)
                    .add(HEADER_COOP)
                    .add(HEADER_CORP),
            )
            .wrap(Governor::new(&governor_conf))
            .wrap(Compress::default())
            .wrap(NormalizePath::trim())
            .service(
                // No directory listing: assets are served individually.
                fs::Files::new(STATIC_ROUTE, STATIC_DIR_PATH)
                    .use_last_modified(true)
                    .use_etag(true)
                    .prefer_utf8(true),
            )
            .route("/", web::get().to(handlers::index))
            .route("/health", web::get().to(handlers::health))
            .service(
                web::scope("/api").service(
                    web::scope("/documents")
                        .route("", web::get().to(handlers::get_documents))
                        .route("", web::post().to(handlers::create_document))
                        .route("/upload", web::post().to(handlers::upload_file))
                        .route("/{id}", web::get().to(handlers::get_document))
                        .route("/{id}", web::put().to(handlers::update_document))
                        .route("/{id}", web::delete().to(handlers::delete_document))
                        .route("/{id}/shares", web::get().to(handlers::list_shares))
                        .route("/{id}/shares", web::post().to(handlers::create_share))
                        .route(
                            "/{id}/shares/{user_id}",
                            web::delete().to(handlers::delete_share),
                        ),
                ),
            )
    })
    //>> Against slowloris-style attacks.
    .max_connections(SERVER_MAX_CONNECTIONS)
    .client_request_timeout(Duration::from_secs(CLIENT_REQUEST_TIMEOUT_SECS))
    .keep_alive(Duration::from_secs(KEEP_ALIVE_SECS))
    //<< Against slowloris-style attacks.
    .workers(SERVER_WORKERS)
    .bind(&bind_address)?
    .run()
    .await
}

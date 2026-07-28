use actix_web::{web, App, HttpServer};
use actix_cors::Cors;
use actix_files as fs;
use tera::Tera;
use std::env;

mod models;
mod handlers;
mod db;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env::set_var("RUST_LOG", "info");
    env_logger::init();

    // Get MongoDB connection string from environment or use default
    let mongo_url = env::var("MONGODB_URL")
        .unwrap_or_else(|_| "mongodb://localhost:27017".to_string());

    // Create MongoDB connection
    let db = db::Db::new(&mongo_url)
        .await
        .expect("Failed to connect to MongoDB");

    // Initialize Tera templates
    let tera = Tera::new("templates/**/*").unwrap();

    let bind_address = env::var("BIND_ADDRESS")
        .unwrap_or_else(|_| "127.0.0.1:17777".to_string());

    println!("Server starting at http://{}", bind_address);
    println!("Connected to MongoDB at {}", mongo_url);

    HttpServer::new(move || {
        let cors = Cors::permissive();

        App::new()
            .app_data(web::Data::new(db.clone()))
            .app_data(web::Data::new(tera.clone()))
            .wrap(cors)
            .service(fs::Files::new("/static", "./static").show_files_listing())
            .route("/", web::get().to(handlers::index))
            .service(
                web::scope("/api")
                    .service(
                        web::scope("/documents")
                            .route("", web::get().to(handlers::get_documents))
                            .route("", web::post().to(handlers::create_document))
                            .route("/upload", web::post().to(handlers::upload_file))
                            .route("/{id}", web::get().to(handlers::get_document))
                            .route("/{id}", web::put().to(handlers::update_document))
                            .route("/{id}", web::delete().to(handlers::delete_document))
                    )
            )
    })
    .bind(&bind_address)?
    .run()
    .await
}

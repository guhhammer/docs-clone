use actix_web::{web, HttpResponse, Responder};
use actix_multipart::Multipart;
use tera::Tera;
use crate::models::{CreateDocumentRequest, UpdateDocumentRequest};
use crate::db;
use futures::TryStreamExt;

pub async fn index(tera: web::Data<Tera>) -> impl Responder {
    let context = tera::Context::new();
    
    match tera.render("index.html", &context) {
        Ok(rendered) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(rendered),
        Err(e) => {
            eprintln!("Template error: {}", e);
            HttpResponse::InternalServerError().body("Template error")
        }
    }
}

pub async fn get_documents(
    db: web::Data<db::Db>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let owner_id = query.get("owner_id").cloned();

    match db::get_documents(&db, owner_id).await {
        Ok(documents) => HttpResponse::Ok().json(documents),
        Err(e) => {
            eprintln!("Error fetching documents: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fetch documents"
            }))
        }
    }
}

pub async fn get_document(
    db: web::Data<db::Db>,
    path: web::Path<String>,
) -> impl Responder {
    let id = path.into_inner();

    match db::get_document(&db, &id).await {
        Ok(Some(document)) => HttpResponse::Ok().json(document),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Document not found"
        })),
        Err(e) => {
            eprintln!("Error fetching document: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fetch document"
            }))
        }
    }
}

pub async fn create_document(
    db: web::Data<db::Db>,
    req: web::Json<CreateDocumentRequest>,
) -> impl Responder {
    match db::create_document(
        &db,
        req.title.clone(),
        req.content.clone(),
        req.owner_id.clone(),
    ).await {
        Ok(document) => HttpResponse::Created().json(document),
        Err(e) => {
            eprintln!("Error creating document: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to create document"
            }))
        }
    }
}

pub async fn update_document(
    db: web::Data<db::Db>,
    path: web::Path<String>,
    req: web::Json<UpdateDocumentRequest>,
) -> impl Responder {
    let id = path.into_inner();
    eprintln!("Updating document {}: title={:?}, content={:?}", id, req.title, req.content);

    match db::update_document(
        &db,
        &id,
        req.title.clone(),
        req.content.clone(),
    ).await {
        Ok(Some(document)) => HttpResponse::Ok().json(document),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Document not found"
        })),
        Err(e) => {
            eprintln!("Error updating document: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to update document"
            }))
        }
    }
}

pub async fn delete_document(
    db: web::Data<db::Db>,
    path: web::Path<String>,
) -> impl Responder {
    let id = path.into_inner();

    match db::delete_document(&db, &id).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Document not found"
        })),
        Err(e) => {
            eprintln!("Error deleting document: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to delete document"
            }))
        }
    }
}

pub async fn upload_file(
    db: web::Data<db::Db>,
    mut payload: Multipart,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let owner_id = query.get("owner_id").cloned().unwrap_or_else(|| "user1".to_string());
    
    let mut file_name = String::new();
    let mut file_content = String::new();
    
    while let Some(mut field) = payload.try_next().await.unwrap() {
        let content_disposition = field.content_disposition();
        
        if let Some(name) = content_disposition.get_name() {
            if name == "file" {
                file_name = content_disposition
                    .get_filename()
                    .unwrap_or("uploaded_file")
                    .to_string();
                
                let mut content = String::new();
                while let Some(chunk) = field.try_next().await.unwrap() {
                    content.push_str(&std::str::from_utf8(&chunk).unwrap_or(""));
                }
                file_content = content;
            }
        }
    }
    
    // Extract title from filename (remove extension)
    let title = if let Some(pos) = file_name.rfind('.') {
        file_name[..pos].to_string()
    } else {
        file_name.clone()
    };
    
    match db::create_document(&db, title, file_content, owner_id).await {
        Ok(document) => HttpResponse::Created().json(document),
        Err(e) => {
            eprintln!("Error creating document from upload: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to create document from upload"
            }))
        }
    }
}

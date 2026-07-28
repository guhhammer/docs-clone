use actix_web::{web, HttpResponse, Responder};
use actix_multipart::Multipart;
use tera::Tera;
use crate::models::{CreateDocumentRequest, DocumentResponse, UpdateDocumentRequest};
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
        Ok(documents) => {
            let body: Vec<DocumentResponse> =
                documents.into_iter().map(DocumentResponse::from).collect();
            HttpResponse::Ok().json(body)
        }
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
        Ok(Some(document)) => HttpResponse::Ok().json(DocumentResponse::from(document)),
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
        Ok(document) => HttpResponse::Created().json(DocumentResponse::from(document)),
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
    match db::update_document(
        &db,
        &id,
        req.title.clone(),
        req.content.clone(),
    ).await {
        Ok(Some(document)) => HttpResponse::Ok().json(DocumentResponse::from(document)),
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

const ALLOWED_UPLOAD_EXTENSIONS: [&str; 2] = ["txt", "md"];

pub async fn upload_file(
    db: web::Data<db::Db>,
    mut payload: Multipart,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let owner_id = query.get("owner_id").cloned().unwrap_or_else(|| "user1".to_string());

    let mut file_name = String::new();
    let mut file_bytes: Vec<u8> = Vec::new();

    while let Ok(Some(mut field)) = payload.try_next().await {
        let is_file_field = field
            .content_disposition()
            .get_name()
            .map(|name| name == "file")
            .unwrap_or(false);

        if !is_file_field {
            continue;
        }

        file_name = field
            .content_disposition()
            .get_filename()
            .unwrap_or("uploaded_file")
            .to_string();

        while let Ok(Some(chunk)) = field.try_next().await {
            file_bytes.extend_from_slice(&chunk);
        }
    }

    if file_bytes.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "No file content received"
        }));
    }

    let extension = file_name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_lowercase())
        .unwrap_or_default();

    if !ALLOWED_UPLOAD_EXTENSIONS.contains(&extension.as_str()) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Unsupported file type. Only .txt and .md files can be imported."
        }));
    }

    let plain_text = match String::from_utf8(file_bytes) {
        Ok(text) => text,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "File must be UTF-8 encoded text"
            }))
        }
    };

    let title = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem.to_string())
        .unwrap_or_else(|| file_name.clone());

    let content = text_to_html(&plain_text);

    match db::create_document(&db, title, content, owner_id).await {
        Ok(document) => HttpResponse::Created().json(DocumentResponse::from(document)),
        Err(e) => {
            eprintln!("Error creating document from upload: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to create document from upload"
            }))
        }
    }
}

/// Converts imported plain text / lightweight markdown into the HTML the
/// rich-text editor understands, so line structure survives the round-trip.
fn text_to_html(text: &str) -> String {
    text.lines()
        .map(|line| {
            let escaped = line
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            let trimmed = escaped.trim_start();

            if trimmed.is_empty() {
                "<p><br></p>".to_string()
            } else if let Some(rest) = trimmed.strip_prefix("### ") {
                format!("<h3>{}</h3>", rest)
            } else if let Some(rest) = trimmed.strip_prefix("## ") {
                format!("<h2>{}</h2>", rest)
            } else if let Some(rest) = trimmed.strip_prefix("# ") {
                format!("<h1>{}</h1>", rest)
            } else if let Some(rest) = trimmed
                .strip_prefix("- ")
                .or_else(|| trimmed.strip_prefix("* "))
            {
                format!("<ul><li>{}</li></ul>", rest)
            } else {
                format!("<p>{}</p>", escaped)
            }
        })
        .collect::<Vec<String>>()
        .join("")
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Document, DocumentResponse};
    use chrono::Utc;

    #[test]
    fn text_to_html_preserves_structure() {
        let html = text_to_html("# Title\n\n- one\nplain <tag>");
        assert_eq!(
            html,
            "<h1>Title</h1><p><br></p><ul><li>one</li></ul><p>plain &lt;tag&gt;</p>"
        );
    }

    #[test]
    fn api_response_exposes_id_field() {
        let now = Utc::now();
        let document = Document {
            id: "abc-123".to_string(),
            title: "Doc".to_string(),
            content: "<p>hi</p>".to_string(),
            owner_id: "user1".to_string(),
            created_at: now,
            updated_at: now,
        };

        let json = serde_json::to_value(DocumentResponse::from(document)).unwrap();
        assert_eq!(json["id"], "abc-123");
        assert_eq!(json["content"], "<p>hi</p>");
        assert!(json.get("_id").is_none());
    }
}

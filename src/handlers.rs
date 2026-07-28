//! HTTP route handlers.
//!
//! Identity is mocked: the caller declares who it is via the `X-User-Id` header
//! (or a `user_id` query parameter as a fallback for curl/demo use). That value
//! is validated before use and is the only basis for authorization decisions —
//! ownership and share records are always re-checked server side, never trusted
//! from the client payload.

use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use futures::TryStreamExt;
use tera::Tera;

use crate::compile_config::{
    ALLOWED_UPLOAD_EXTENSIONS, MAX_UPLOAD_BYTES, TEMPLATE_INDEX,
};
use crate::db;
use crate::models::{
    Access, CreateDocumentRequest, CreateShareRequest, DocumentResponse, ShareResponse,
    UpdateDocumentRequest,
};
use crate::security::{
    safe_for_log, sanitize_content, sanitize_title, validate_document_id, validate_permission,
    validate_user_id, ValidationError,
};

type Query = web::Query<std::collections::HashMap<String, String>>;

fn bad_request(message: &str) -> HttpResponse {
    HttpResponse::BadRequest().json(serde_json::json!({ "error": message }))
}

fn forbidden(message: &str) -> HttpResponse {
    HttpResponse::Forbidden().json(serde_json::json!({ "error": message }))
}

fn not_found(message: &str) -> HttpResponse {
    HttpResponse::NotFound().json(serde_json::json!({ "error": message }))
}

/// Generic 500 that never leaks internal error details to the client.
fn internal_error(context: &str, error: impl std::fmt::Display) -> HttpResponse {
    eprintln!("{}: {}", context, error);
    HttpResponse::InternalServerError()
        .json(serde_json::json!({ "error": "Internal server error" }))
}

/// Resolve the calling user from `X-User-Id`, falling back to `?user_id=`.
fn caller_id(req: &HttpRequest, query: &Query) -> Result<String, ValidationError> {
    let raw = req
        .headers()
        .get("X-User-Id")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string())
        .or_else(|| query.get("user_id").cloned())
        .or_else(|| query.get("owner_id").cloned())
        .unwrap_or_default();

    validate_user_id(&raw)
}

/// Determine what `user_id` may do with `document`.
async fn resolve_access(
    db: &db::Db,
    document: &crate::models::Document,
    user_id: &str,
) -> Result<Option<Access>, mongodb::error::Error> {
    if document.owner_id == user_id {
        return Ok(Some(Access::Owner));
    }

    Ok(db::get_share(db, &document.id, user_id)
        .await?
        .and_then(|share| match share.permission.as_str() {
            "edit" => Some(Access::Edit),
            "view" => Some(Access::View),
            _ => None,
        }))
}

pub async fn index(tera: web::Data<Tera>) -> impl Responder {
    match tera.render(TEMPLATE_INDEX, &tera::Context::new()) {
        Ok(rendered) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(rendered),
        Err(e) => internal_error("Template error", e),
    }
}

/// Health probe for deployment checks.
pub async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({ "status": "ok" }))
}

/// Lists documents owned by the caller and documents shared with the caller.
pub async fn get_documents(
    db: web::Data<db::Db>,
    req: HttpRequest,
    query: Query,
) -> impl Responder {
    let user_id = match caller_id(&req, &query) {
        Ok(id) => id,
        Err(e) => return bad_request(&e.0),
    };

    let owned = match db::get_owned_documents(&db, &user_id).await {
        Ok(docs) => docs,
        Err(e) => return internal_error("Error fetching owned documents", e),
    };

    let shared = match db::get_shared_documents(&db, &user_id).await {
        Ok(docs) => docs,
        Err(e) => return internal_error("Error fetching shared documents", e),
    };

    let mut owned_response = Vec::with_capacity(owned.len());
    for document in owned {
        let count = match db::get_shares_for_document(&db, &document.id).await {
            Ok(shares) => shares.len(),
            Err(e) => return internal_error("Error fetching shares", e),
        };
        owned_response.push(DocumentResponse::new(document, Access::Owner, count));
    }

    let shared_response: Vec<DocumentResponse> = shared
        .into_iter()
        .map(|(document, permission)| {
            let access = if permission == "edit" {
                Access::Edit
            } else {
                Access::View
            };
            DocumentResponse::new(document, access, 0)
        })
        .collect();

    HttpResponse::Ok().json(serde_json::json!({
        "owned": owned_response,
        "shared": shared_response,
    }))
}

pub async fn get_document(
    db: web::Data<db::Db>,
    req: HttpRequest,
    path: web::Path<String>,
    query: Query,
) -> impl Responder {
    let user_id = match caller_id(&req, &query) {
        Ok(id) => id,
        Err(e) => return bad_request(&e.0),
    };
    let id = match validate_document_id(&path.into_inner()) {
        Ok(id) => id,
        Err(e) => return bad_request(&e.0),
    };

    let document = match db::get_document(&db, &id).await {
        Ok(Some(document)) => document,
        Ok(None) => return not_found("Document not found"),
        Err(e) => return internal_error("Error fetching document", e),
    };

    let access = match resolve_access(&db, &document, &user_id).await {
        Ok(Some(access)) => access,
        // Do not disclose the existence of documents the caller cannot see.
        Ok(None) => return not_found("Document not found"),
        Err(e) => return internal_error("Error resolving access", e),
    };

    let count = if access.is_owner() {
        match db::get_shares_for_document(&db, &id).await {
            Ok(shares) => shares.len(),
            Err(e) => return internal_error("Error fetching shares", e),
        }
    } else {
        0
    };

    HttpResponse::Ok().json(DocumentResponse::new(document, access, count))
}

pub async fn create_document(
    db: web::Data<db::Db>,
    req: HttpRequest,
    query: Query,
    body: web::Json<CreateDocumentRequest>,
) -> impl Responder {
    let owner_id = match caller_id(&req, &query) {
        Ok(id) => id,
        Err(e) => return bad_request(&e.0),
    };

    let title = match sanitize_title(&body.title) {
        Ok(title) => title,
        Err(e) => return bad_request(&e.0),
    };
    let content = match sanitize_content(&body.content) {
        Ok(content) => content,
        Err(e) => return bad_request(&e.0),
    };

    match db::create_document(&db, title, content, owner_id).await {
        Ok(document) => {
            HttpResponse::Created().json(DocumentResponse::new(document, Access::Owner, 0))
        }
        Err(e) => internal_error("Error creating document", e),
    }
}

pub async fn update_document(
    db: web::Data<db::Db>,
    req: HttpRequest,
    path: web::Path<String>,
    query: Query,
    body: web::Json<UpdateDocumentRequest>,
) -> impl Responder {
    let user_id = match caller_id(&req, &query) {
        Ok(id) => id,
        Err(e) => return bad_request(&e.0),
    };
    let id = match validate_document_id(&path.into_inner()) {
        Ok(id) => id,
        Err(e) => return bad_request(&e.0),
    };

    let document = match db::get_document(&db, &id).await {
        Ok(Some(document)) => document,
        Ok(None) => return not_found("Document not found"),
        Err(e) => return internal_error("Error fetching document", e),
    };

    let access = match resolve_access(&db, &document, &user_id).await {
        Ok(Some(access)) => access,
        Ok(None) => return not_found("Document not found"),
        Err(e) => return internal_error("Error resolving access", e),
    };

    if !access.can_edit() {
        return forbidden("You have view-only access to this document");
    }

    let title = match body.title.as_deref().map(sanitize_title).transpose() {
        Ok(title) => title,
        Err(e) => return bad_request(&e.0),
    };
    let content = match body.content.as_deref().map(sanitize_content).transpose() {
        Ok(content) => content,
        Err(e) => return bad_request(&e.0),
    };

    if title.is_none() && content.is_none() {
        return bad_request("Nothing to update");
    }

    let outcome = db::update_document(&db, &id, title, content, body.revision).await;

    match outcome {
        Ok(db::UpdateOutcome::Updated(document)) => {
            let count = shared_with_count(&db, &id, access).await;
            HttpResponse::Ok().json(DocumentResponse::new(document, access, count))
        }
        // Someone else saved while this client was editing. Return 409 together
        // with the current server-side document so the UI can offer the user a
        // choice instead of losing one of the two versions.
        Ok(db::UpdateOutcome::Conflict(current)) => {
            let count = shared_with_count(&db, &id, access).await;
            HttpResponse::Conflict().json(serde_json::json!({
                "error": "This document was changed by someone else since you opened it",
                "code": "revision_conflict",
                "document": DocumentResponse::new(current, access, count),
            }))
        }
        Ok(db::UpdateOutcome::Missing) => not_found("Document not found"),
        Err(e) => internal_error("Error updating document", e),
    }
}

async fn shared_with_count(db: &db::Db, id: &str, access: Access) -> usize {
    if !access.is_owner() {
        return 0;
    }
    db::get_shares_for_document(db, id)
        .await
        .map(|shares| shares.len())
        .unwrap_or(0)
}

/// Only the owner may delete a document.
pub async fn delete_document(
    db: web::Data<db::Db>,
    req: HttpRequest,
    path: web::Path<String>,
    query: Query,
) -> impl Responder {
    let user_id = match caller_id(&req, &query) {
        Ok(id) => id,
        Err(e) => return bad_request(&e.0),
    };
    let id = match validate_document_id(&path.into_inner()) {
        Ok(id) => id,
        Err(e) => return bad_request(&e.0),
    };

    let document = match db::get_document(&db, &id).await {
        Ok(Some(document)) => document,
        Ok(None) => return not_found("Document not found"),
        Err(e) => return internal_error("Error fetching document", e),
    };

    if document.owner_id != user_id {
        return match resolve_access(&db, &document, &user_id).await {
            Ok(Some(_)) => forbidden("Only the owner can delete this document"),
            Ok(None) => not_found("Document not found"),
            Err(e) => internal_error("Error resolving access", e),
        };
    }

    match db::delete_document(&db, &id).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => not_found("Document not found"),
        Err(e) => internal_error("Error deleting document", e),
    }
}

// ---------------------------------------------------------------------------
// Sharing
// ---------------------------------------------------------------------------

/// Lists who a document is shared with. Owner only.
pub async fn list_shares(
    db: web::Data<db::Db>,
    req: HttpRequest,
    path: web::Path<String>,
    query: Query,
) -> impl Responder {
    let user_id = match caller_id(&req, &query) {
        Ok(id) => id,
        Err(e) => return bad_request(&e.0),
    };
    let id = match validate_document_id(&path.into_inner()) {
        Ok(id) => id,
        Err(e) => return bad_request(&e.0),
    };

    match db::get_document(&db, &id).await {
        Ok(Some(document)) if document.owner_id == user_id => {}
        Ok(Some(_)) => return forbidden("Only the owner can manage sharing"),
        Ok(None) => return not_found("Document not found"),
        Err(e) => return internal_error("Error fetching document", e),
    }

    match db::get_shares_for_document(&db, &id).await {
        Ok(shares) => {
            let body: Vec<ShareResponse> = shares.into_iter().map(ShareResponse::from).collect();
            HttpResponse::Ok().json(body)
        }
        Err(e) => internal_error("Error fetching shares", e),
    }
}

/// Grants access to another user. Owner only.
pub async fn create_share(
    db: web::Data<db::Db>,
    req: HttpRequest,
    path: web::Path<String>,
    query: Query,
    body: web::Json<CreateShareRequest>,
) -> impl Responder {
    let owner_id = match caller_id(&req, &query) {
        Ok(id) => id,
        Err(e) => return bad_request(&e.0),
    };
    let id = match validate_document_id(&path.into_inner()) {
        Ok(id) => id,
        Err(e) => return bad_request(&e.0),
    };
    let grantee = match validate_user_id(&body.user_id) {
        Ok(id) => id,
        Err(e) => return bad_request(&e.0),
    };
    let permission = match validate_permission(&body.permission) {
        Ok(permission) => permission,
        Err(e) => return bad_request(&e.0),
    };

    match db::get_document(&db, &id).await {
        Ok(Some(document)) if document.owner_id == owner_id => {}
        Ok(Some(_)) => return forbidden("Only the owner can manage sharing"),
        Ok(None) => return not_found("Document not found"),
        Err(e) => return internal_error("Error fetching document", e),
    }

    if grantee == owner_id {
        return bad_request("You already own this document");
    }

    match db::upsert_share(&db, &id, &owner_id, &grantee, &permission).await {
        Ok(share) => HttpResponse::Created().json(ShareResponse::from(share)),
        Err(e) => internal_error("Error creating share", e),
    }
}

/// Revokes access. Owner only.
pub async fn delete_share(
    db: web::Data<db::Db>,
    req: HttpRequest,
    path: web::Path<(String, String)>,
    query: Query,
) -> impl Responder {
    let owner_id = match caller_id(&req, &query) {
        Ok(id) => id,
        Err(e) => return bad_request(&e.0),
    };
    let (raw_id, raw_user) = path.into_inner();
    let id = match validate_document_id(&raw_id) {
        Ok(id) => id,
        Err(e) => return bad_request(&e.0),
    };
    let grantee = match validate_user_id(&raw_user) {
        Ok(id) => id,
        Err(e) => return bad_request(&e.0),
    };

    match db::get_document(&db, &id).await {
        Ok(Some(document)) if document.owner_id == owner_id => {}
        Ok(Some(_)) => return forbidden("Only the owner can manage sharing"),
        Ok(None) => return not_found("Document not found"),
        Err(e) => return internal_error("Error fetching document", e),
    }

    match db::delete_share(&db, &id, &grantee).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => not_found("Share not found"),
        Err(e) => internal_error("Error deleting share", e),
    }
}

// ---------------------------------------------------------------------------
// Upload / import
// ---------------------------------------------------------------------------

/// Imports a `.txt` or `.md` file as a new document owned by the caller.
pub async fn upload_file(
    db: web::Data<db::Db>,
    req: HttpRequest,
    query: Query,
    mut payload: Multipart,
) -> impl Responder {
    let owner_id = match caller_id(&req, &query) {
        Ok(id) => id,
        Err(e) => return bad_request(&e.0),
    };

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

        // The filename is only used to derive a title and an extension; it never
        // touches the filesystem, and control characters are stripped.
        file_name = field
            .content_disposition()
            .get_filename()
            .unwrap_or("uploaded_file")
            .to_string();

        while let Ok(Some(chunk)) = field.try_next().await {
            if file_bytes.len() + chunk.len() > MAX_UPLOAD_BYTES {
                return bad_request("File is too large (limit 1 MB)");
            }
            file_bytes.extend_from_slice(&chunk);
        }
    }

    if file_bytes.is_empty() {
        return bad_request("No file content received");
    }

    let file_name = safe_for_log(&file_name);
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();

    if !ALLOWED_UPLOAD_EXTENSIONS.contains(&extension.as_str()) {
        return bad_request("Unsupported file type. Only .txt and .md files can be imported.");
    }

    let plain_text = match String::from_utf8(file_bytes) {
        Ok(text) => text,
        Err(_) => return bad_request("File must be UTF-8 encoded text"),
    };

    let raw_title = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem.to_string())
        .unwrap_or_else(|| file_name.clone());

    let title = match sanitize_title(&raw_title) {
        Ok(title) => title,
        Err(e) => return bad_request(&e.0),
    };
    // `text_to_html` escapes the source text, and the sanitizer is applied on top
    // so an imported file can never introduce markup of its own.
    let content = match sanitize_content(&text_to_html(&plain_text)) {
        Ok(content) => content,
        Err(e) => return bad_request(&e.0),
    };

    match db::create_document(&db, title, content, owner_id).await {
        Ok(document) => {
            HttpResponse::Created().json(DocumentResponse::new(document, Access::Owner, 0))
        }
        Err(e) => internal_error("Error creating document from upload", e),
    }
}

/// Converts imported plain text / lightweight markdown into the HTML the
/// rich-text editor understands, so line structure survives the round-trip.
/// All source characters are HTML-escaped first.
fn text_to_html(text: &str) -> String {
    text.lines()
        .map(|line| {
            let escaped = line
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;");
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
    use crate::models::Document;
    use chrono::Utc;

    #[test]
    fn text_to_html_escapes_and_preserves_structure() {
        let html = text_to_html("# Title\n\n- one\nplain <script>x</script>");
        assert_eq!(
            html,
            "<h1>Title</h1><p><br></p><ul><li>one</li></ul>\
             <p>plain &lt;script&gt;x&lt;/script&gt;</p>"
        );
    }

    #[test]
    fn api_response_exposes_id_and_access() {
        let now = Utc::now();
        let document = Document {
            id: "abc-123".to_string(),
            title: "Doc".to_string(),
            content: "<p>hi</p>".to_string(),
            owner_id: "user1".to_string(),
            created_at: now,
            updated_at: now,
            revision: 3,
        };

        let json = serde_json::to_value(DocumentResponse::new(document, Access::View, 0)).unwrap();
        assert_eq!(json["id"], "abc-123");
        assert_eq!(json["content"], "<p>hi</p>");
        assert_eq!(json["access"], "view");
        assert_eq!(json["revision"], 3);
        assert!(json.get("_id").is_none());
    }

    #[test]
    fn access_levels_gate_writes() {
        assert!(Access::Owner.can_edit() && Access::Owner.is_owner());
        assert!(Access::Edit.can_edit() && !Access::Edit.is_owner());
        assert!(!Access::View.can_edit() && !Access::View.is_owner());
    }
}

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use mongodb::bson::serde_helpers::chrono_datetime_as_bson_datetime;

/// Persisted shape of a document. `id` maps to the MongoDB `_id` key and the
/// timestamps are stored as native BSON dates so they can be sorted/queried.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Document {
    #[serde(rename = "_id")]
    pub id: String,
    pub title: String,
    pub content: String,
    pub owner_id: String,
    #[serde(with = "chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono_datetime_as_bson_datetime")]
    pub updated_at: DateTime<Utc>,
}

/// Shape returned by the HTTP API. Exposes `id` (not `_id`) so the client can
/// address a document with a stable, predictable key.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DocumentResponse {
    pub id: String,
    pub title: String,
    pub content: String,
    pub owner_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Document> for DocumentResponse {
    fn from(doc: Document) -> Self {
        DocumentResponse {
            id: doc.id,
            title: doc.title,
            content: doc.content,
            owner_id: doc.owner_id,
            created_at: doc.created_at,
            updated_at: doc.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateDocumentRequest {
    pub title: String,
    #[serde(default)]
    pub content: String,
    pub owner_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDocumentRequest {
    pub title: Option<String>,
    pub content: Option<String>,
}

use chrono::{DateTime, Utc};
use mongodb::bson::serde_helpers::chrono_datetime_as_bson_datetime;
use serde::{Deserialize, Serialize};

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

/// Persisted share record: one row per (document, grantee) pair.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Share {
    #[serde(rename = "_id")]
    pub id: String,
    pub document_id: String,
    pub owner_id: String,
    pub user_id: String,
    /// `view` or `edit`.
    pub permission: String,
    #[serde(with = "chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
}

/// How the requesting user may act on a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    Owner,
    Edit,
    View,
}

impl Access {
    pub fn can_edit(self) -> bool {
        matches!(self, Access::Owner | Access::Edit)
    }

    pub fn is_owner(self) -> bool {
        matches!(self, Access::Owner)
    }
}

/// Shape returned by the HTTP API. Exposes `id` (not `_id`) so the client can
/// address a document with a stable, predictable key, plus the caller's access
/// level so the UI can distinguish owned from shared documents.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DocumentResponse {
    pub id: String,
    pub title: String,
    pub content: String,
    pub owner_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub access: String,
    pub shared_with_count: usize,
}

impl DocumentResponse {
    pub fn new(doc: Document, access: Access, shared_with_count: usize) -> Self {
        let access = match access {
            Access::Owner => "owner",
            Access::Edit => "edit",
            Access::View => "view",
        };

        DocumentResponse {
            id: doc.id,
            title: doc.title,
            content: doc.content,
            owner_id: doc.owner_id,
            created_at: doc.created_at,
            updated_at: doc.updated_at,
            access: access.to_string(),
            shared_with_count,
        }
    }
}

/// Share record as exposed by the API.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ShareResponse {
    pub user_id: String,
    pub permission: String,
    pub created_at: DateTime<Utc>,
}

impl From<Share> for ShareResponse {
    fn from(share: Share) -> Self {
        ShareResponse {
            user_id: share.user_id,
            permission: share.permission,
            created_at: share.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateDocumentRequest {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDocumentRequest {
    pub title: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateShareRequest {
    pub user_id: String,
    #[serde(default = "default_permission")]
    pub permission: String,
}

fn default_permission() -> String {
    "edit".to_string()
}

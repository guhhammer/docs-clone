//! Data access layer.
//!
//! All queries are built with typed `doc!` literals and pre-validated values
//! (see `crate::security`), so no user-supplied string is ever interpolated into
//! a query document or interpreted as a BSON operator.

use futures::stream::StreamExt;
use mongodb::{
    bson::{doc, DateTime as BsonDateTime},
    options::{ClientOptions, IndexOptions},
    Client, Collection, IndexModel,
};
use std::time::Duration;

use crate::compile_config::{
    COLLECTION_DOCUMENTS, COLLECTION_SHARES, DB_NAME, DB_SERVER_SELECTION_TIMEOUT_SECS,
};
use crate::models::{Document, Share};
use chrono::Utc;

#[derive(Clone)]
pub struct Db {
    client: Client,
    db_name: String,
}

impl Db {
    pub async fn new(connection_string: &str) -> Result<Self, mongodb::error::Error> {
        let mut options = ClientOptions::parse(connection_string).await?;
        options.server_selection_timeout =
            Some(Duration::from_secs(DB_SERVER_SELECTION_TIMEOUT_SECS));

        let client = Client::with_options(options)?;
        let db = Db {
            client,
            db_name: DB_NAME.to_string(),
        };
        db.ensure_indexes().await?;
        Ok(db)
    }

    fn documents(&self) -> Collection<Document> {
        self.client
            .database(&self.db_name)
            .collection::<Document>(COLLECTION_DOCUMENTS)
    }

    fn shares(&self) -> Collection<Share> {
        self.client
            .database(&self.db_name)
            .collection::<Share>(COLLECTION_SHARES)
    }

    /// Indexes that back the list queries and enforce one share per grantee.
    async fn ensure_indexes(&self) -> Result<(), mongodb::error::Error> {
        self.documents()
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "owner_id": 1, "updated_at": -1 })
                    .build(),
            )
            .await?;

        self.shares()
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "document_id": 1, "user_id": 1 })
                    .options(IndexOptions::builder().unique(true).build())
                    .build(),
            )
            .await?;

        self.shares()
            .create_index(
                IndexModel::builder().keys(doc! { "user_id": 1 }).build(),
            )
            .await?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------

pub async fn create_document(
    db: &Db,
    title: String,
    content: String,
    owner_id: String,
) -> Result<Document, mongodb::error::Error> {
    let now = Utc::now();
    let document = Document {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        content,
        owner_id,
        created_at: now,
        updated_at: now,
        revision: 1,
    };

    db.documents().insert_one(&document).await?;
    Ok(document)
}

/// Documents owned by `owner_id`, most recently edited first.
pub async fn get_owned_documents(
    db: &Db,
    owner_id: &str,
) -> Result<Vec<Document>, mongodb::error::Error> {
    let mut cursor = db
        .documents()
        .find(doc! { "owner_id": owner_id })
        .sort(doc! { "updated_at": -1 })
        .await?;

    let mut documents = Vec::new();
    while let Some(document) = cursor.next().await {
        documents.push(document?);
    }
    Ok(documents)
}

/// Documents shared with `user_id`, paired with the granted permission.
pub async fn get_shared_documents(
    db: &Db,
    user_id: &str,
) -> Result<Vec<(Document, String)>, mongodb::error::Error> {
    let shares = get_shares_for_user(db, user_id).await?;
    if shares.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<String> = shares.iter().map(|s| s.document_id.clone()).collect();
    let mut cursor = db
        .documents()
        .find(doc! { "_id": { "$in": ids } })
        .sort(doc! { "updated_at": -1 })
        .await?;

    let mut documents = Vec::new();
    while let Some(document) = cursor.next().await {
        let document = document?;
        if let Some(share) = shares.iter().find(|s| s.document_id == document.id) {
            let permission = share.permission.clone();
            documents.push((document, permission));
        }
    }
    Ok(documents)
}

pub async fn get_document(db: &Db, id: &str) -> Result<Option<Document>, mongodb::error::Error> {
    db.documents().find_one(doc! { "_id": id }).await
}

/// Result of a conditional update.
pub enum UpdateOutcome {
    /// The write applied; carries the stored document with its new revision.
    Updated(Document),
    /// Another writer got there first; carries the current server-side document
    /// so the caller can show the user what is actually stored.
    Conflict(Document),
    /// The document no longer exists.
    Missing,
}

/// Applies an edit with optimistic concurrency control.
///
/// When `expected_revision` is `Some`, the update filter also matches on the
/// stored `revision`, so a save based on stale content cannot overwrite a newer
/// one - it reports `Conflict` instead. `revision` is incremented atomically by
/// the same update, in a single round trip, so two concurrent writers can never
/// both succeed against the same base revision.
pub async fn update_document(
    db: &Db,
    id: &str,
    title: Option<String>,
    content: Option<String>,
    expected_revision: Option<u64>,
) -> Result<UpdateOutcome, mongodb::error::Error> {
    let now_bson = BsonDateTime::from_system_time(Utc::now().into());
    let mut set_doc = doc! { "updated_at": now_bson };

    if let Some(t) = title {
        set_doc.insert("title", t);
    }
    if let Some(c) = content {
        set_doc.insert("content", c);
    }

    let mut filter = doc! { "_id": id };
    if let Some(revision) = expected_revision {
        filter.insert("revision", revision as i64);
    }

    let result = db
        .documents()
        .update_one(
            filter,
            doc! { "$set": set_doc, "$inc": { "revision": 1_i64 } },
        )
        .await?;

    if result.matched_count == 0 {
        // Either the document is gone, or its revision moved on: fetch it once
        // to tell those two cases apart.
        return match get_document(db, id).await? {
            Some(current) => Ok(UpdateOutcome::Conflict(current)),
            None => Ok(UpdateOutcome::Missing),
        };
    }

    match get_document(db, id).await? {
        Some(document) => Ok(UpdateOutcome::Updated(document)),
        None => Ok(UpdateOutcome::Missing),
    }
}

/// Deletes a document and every share pointing at it.
///
/// Runs both deletes inside a single transaction when the deployment supports
/// them (replica set / mongos). On a standalone server transactions are not
/// available, so it falls back to deleting the shares *first*: an interruption
/// then leaves a document with no shares (harmless, still owned and visible)
/// rather than orphan share rows pointing at a document that no longer exists.
/// `cleanup_orphan_shares` sweeps up anything an earlier crash left behind.
pub async fn delete_document(db: &Db, id: &str) -> Result<bool, mongodb::error::Error> {
    match delete_document_in_transaction(db, id).await {
        Ok(deleted) => Ok(deleted),
        Err(e) if is_transaction_unsupported(&e) => {
            db.shares().delete_many(doc! { "document_id": id }).await?;
            let result = db.documents().delete_one(doc! { "_id": id }).await?;
            Ok(result.deleted_count > 0)
        }
        Err(e) => Err(e),
    }
}

async fn delete_document_in_transaction(
    db: &Db,
    id: &str,
) -> Result<bool, mongodb::error::Error> {
    let mut session = db.client.start_session().await?;
    session.start_transaction().await?;

    let shares_result = db
        .shares()
        .delete_many(doc! { "document_id": id })
        .session(&mut session)
        .await;
    if let Err(e) = shares_result {
        let _ = session.abort_transaction().await;
        return Err(e);
    }

    let document_result = db
        .documents()
        .delete_one(doc! { "_id": id })
        .session(&mut session)
        .await;
    let deleted = match document_result {
        Ok(result) => result.deleted_count > 0,
        Err(e) => {
            let _ = session.abort_transaction().await;
            return Err(e);
        }
    };

    session.commit_transaction().await?;
    Ok(deleted)
}

/// True when the error is "this deployment has no transactions" rather than a
/// real failure, so the caller can fall back to the sequential delete.
fn is_transaction_unsupported(error: &mongodb::error::Error) -> bool {
    let text = error.to_string();
    text.contains("Transaction numbers are only allowed")
        || text.contains("Transactions are not supported")
        || text.contains("does not support transactions")
        || text.contains("IllegalOperation")
}

/// Removes share rows whose document no longer exists. Called at startup so a
/// crash between the two deletes of a previous run cannot leave a grantee with a
/// share pointing at nothing. Returns the number of rows removed.
pub async fn cleanup_orphan_shares(db: &Db) -> Result<u64, mongodb::error::Error> {
    let mut cursor = db.documents().find(doc! {}).await?;
    let mut live_ids: Vec<String> = Vec::new();
    while let Some(document) = cursor.next().await {
        live_ids.push(document?.id);
    }

    let result = db
        .shares()
        .delete_many(doc! { "document_id": { "$nin": live_ids } })
        .await?;
    Ok(result.deleted_count)
}

// ---------------------------------------------------------------------------
// Shares
// ---------------------------------------------------------------------------

pub async fn get_shares_for_document(
    db: &Db,
    document_id: &str,
) -> Result<Vec<Share>, mongodb::error::Error> {
    let mut cursor = db
        .shares()
        .find(doc! { "document_id": document_id })
        .await?;

    let mut shares = Vec::new();
    while let Some(share) = cursor.next().await {
        shares.push(share?);
    }
    shares.sort_by(|a, b| a.user_id.cmp(&b.user_id));
    Ok(shares)
}

pub async fn get_shares_for_user(
    db: &Db,
    user_id: &str,
) -> Result<Vec<Share>, mongodb::error::Error> {
    let mut cursor = db.shares().find(doc! { "user_id": user_id }).await?;

    let mut shares = Vec::new();
    while let Some(share) = cursor.next().await {
        shares.push(share?);
    }
    Ok(shares)
}

pub async fn get_share(
    db: &Db,
    document_id: &str,
    user_id: &str,
) -> Result<Option<Share>, mongodb::error::Error> {
    db.shares()
        .find_one(doc! { "document_id": document_id, "user_id": user_id })
        .await
}

/// Grants (or updates) access for `user_id` on `document_id`.
pub async fn upsert_share(
    db: &Db,
    document_id: &str,
    owner_id: &str,
    user_id: &str,
    permission: &str,
) -> Result<Share, mongodb::error::Error> {
    let now = Utc::now();
    let share = Share {
        id: uuid::Uuid::new_v4().to_string(),
        document_id: document_id.to_string(),
        owner_id: owner_id.to_string(),
        user_id: user_id.to_string(),
        permission: permission.to_string(),
        created_at: now,
    };

    let now_bson = BsonDateTime::from_system_time(now.into());
    db.shares()
        .update_one(
            doc! { "document_id": document_id, "user_id": user_id },
            doc! {
                "$set": { "permission": permission, "owner_id": owner_id },
                "$setOnInsert": { "_id": &share.id, "created_at": now_bson },
            },
        )
        .upsert(true)
        .await?;

    match get_share(db, document_id, user_id).await? {
        Some(existing) => Ok(existing),
        None => Ok(share),
    }
}

pub async fn delete_share(
    db: &Db,
    document_id: &str,
    user_id: &str,
) -> Result<bool, mongodb::error::Error> {
    let result = db
        .shares()
        .delete_one(doc! { "document_id": document_id, "user_id": user_id })
        .await?;
    Ok(result.deleted_count > 0)
}

use mongodb::{
    Client,
    Collection,
    bson::{doc, DateTime as BsonDateTime},
    options::FindOptions,
};
use crate::models::Document;
use chrono::Utc;
use futures::stream::StreamExt;

#[derive(Clone)]
pub struct Db {
    client: Client,
    db_name: String,
}

impl Db {
    pub async fn new(connection_string: &str) -> Result<Self, mongodb::error::Error> {
        let client = Client::with_uri_str(connection_string).await?;
        Ok(Db { client, db_name: "docs_clone".to_string() })
    }

    fn documents(&self) -> Collection<Document> {
        self.client.database(&self.db_name).collection::<Document>("documents")
    }
}

pub async fn create_document(
    db: &Db,
    title: String,
    content: String,
    owner_id: String,
) -> Result<Document, mongodb::error::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();

    let document = Document {
        id: id.clone(),
        title,
        content,
        owner_id,
        created_at: now,
        updated_at: now,
    };

    let collection = db.documents();
    collection.insert_one(&document, None).await?;

    Ok(document)
}

pub async fn get_documents(db: &Db, owner_id: Option<String>) -> Result<Vec<Document>, mongodb::error::Error> {
    let collection = db.documents();
    
    let filter = if let Some(owner_id) = owner_id {
        doc! { "owner_id": owner_id }
    } else {
        doc! {}
    };

    let find_options = FindOptions::builder()
        .sort(doc! { "updated_at": -1 })
        .build();
    let mut cursor = collection.find(filter, find_options).await?;

    let mut documents = Vec::new();
    while let Some(document) = cursor.next().await {
        documents.push(document?);
    }

    Ok(documents)
}

pub async fn get_document(db: &Db, id: &str) -> Result<Option<Document>, mongodb::error::Error> {
    let collection = db.documents();
    let document = collection.find_one(doc! { "_id": id }, None).await?;
    Ok(document)
}

pub async fn update_document(
    db: &Db,
    id: &str,
    title: Option<String>,
    content: Option<String>,
) -> Result<Option<Document>, mongodb::error::Error> {
    let collection = db.documents();
    let now = Utc::now();
    let now_bson = BsonDateTime::from_system_time(now.into());

    let mut update_doc = doc! { "updated_at": now_bson };
    
    if let Some(t) = title {
        update_doc.insert("title", t);
    }
    
    if let Some(c) = content {
        eprintln!("Setting content to: {}", c);
        update_doc.insert("content", c);
    }

    eprintln!("Update document: {:?}", update_doc);
    let update = doc! { "$set": update_doc };
    let result = collection.update_one(doc! { "_id": id }, update, None).await?;
    eprintln!("Update result: matched={}, modified={}", result.matched_count, result.modified_count);

    // Fetch and return the updated document
    get_document(db, id).await
}

pub async fn delete_document(db: &Db, id: &str) -> Result<bool, mongodb::error::Error> {
    let collection = db.documents();
    let result = collection.delete_one(doc! { "_id": id }, None).await?;
    Ok(result.deleted_count > 0)
}

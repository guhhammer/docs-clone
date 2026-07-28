# Architecture Note

## Overview

Docs Clone is a lightweight collaborative document editor built with a Rust backend and vanilla JavaScript frontend. The architecture prioritizes simplicity, performance, and ease of deployment while demonstrating full-stack capabilities.

## Technology Choices

### Backend: Rust with Actix-web
- **Why Rust**: Performance, memory safety, and strong type system ensure reliability
- **Why Actix-web**: High-performance async framework with excellent ecosystem support
- **SQLite**: Simple, file-based database requiring no external dependencies - ideal for this scope

### Frontend: Vanilla JavaScript
- **Why Vanilla JS**: No build step required, immediate browser compatibility, sufficient for the required features
- **Contenteditable API**: Native browser API for rich text editing without heavy libraries
- **CSS3**: Modern styling without framework overhead

## System Architecture

```
┌─────────────────┐         ┌─────────────────┐
│   Frontend       │         │   Backend       │
│   (Vanilla JS)   │◄────────►│   (Actix-web)   │
│                 │ HTTP/JSON │                 │
│ - Rich Text     │         │ - REST API      │
│ - File Upload   │         │ - Multipart     │
│ - Auto-save     │         │ - CORS          │
└─────────────────┘         └────────┬────────┘
                                      │
                                      ▼
                             ┌─────────────────┐
                             │   SQLite DB     │
                             │   (docs.db)     │
                             └─────────────────┘
```

## Key Components

### Backend Structure
- **main.rs**: Application entry point, server configuration, routing
- **models.rs**: Data structures (Document, CreateDocumentRequest, UpdateDocumentRequest)
- **handlers.rs**: HTTP request handlers for all API endpoints
- **db.rs**: Database operations (CRUD for documents)
- **migrations/**: SQL schema for database initialization

### Frontend Structure
- **index.html**: Single-page application structure
- **styles.css**: Responsive, modern UI styling
- **app.js**: Client-side logic for document management, rich text editing, file upload

## Data Model

### Document Schema
```sql
CREATE TABLE documents (
    id TEXT PRIMARY KEY,           -- UUID v4
    title TEXT NOT NULL,          -- Document title
    content TEXT NOT NULL,        -- HTML content from editor
    owner_id TEXT NOT NULL,       -- User identifier
    created_at TEXT NOT NULL,     -- ISO 8601 timestamp
    updated_at TEXT NOT NULL      -- ISO 8601 timestamp
);
```

## API Design

### RESTful Endpoints
- **GET /**: Health check
- **GET /api/documents**: List documents (with optional owner_id filter)
- **POST /api/documents**: Create new document
- **GET /api/documents/{id}**: Get specific document
- **PUT /api/documents/{id}**: Update document
- **DELETE /api/documents/{id}**: Delete document
- **POST /api/documents/upload**: Upload file as document

### Response Format
All endpoints return JSON responses with appropriate HTTP status codes (200, 201, 404, 500).

## Prioritization Decisions

### Implemented (Core Features)
1. **Document CRUD**: Essential for any document editor
2. **Rich Text Editing**: Basic formatting (bold, italic, underline, headings, lists) using contenteditable
3. **File Upload**: Import .txt/.md files as documents - practical use case
4. **Persistence**: SQLite for reliable data storage
5. **Auto-save**: Improves UX by preventing data loss

### Intentionally Deferred (Scope Management)
1. **Real-time Collaboration**: Would require WebSockets and operational transformation - complex for timebox
2. **Authentication**: Simple user ID system sufficient for demonstration
3. **Sharing**: Basic sharing model deferred to focus on core editing experience
4. **Version History**: Nice-to-have but not essential for MVP
5. **Export Functionality**: Can be added later without affecting core architecture

## Security Considerations

- **CORS**: Configured permissive for development (should be restricted in production)
- **Input Validation**: Basic validation on document titles and content
- **SQL Injection**: Prevented through SQLx parameterized queries
- **File Upload**: Limited to .txt and .md file types to reduce attack surface

## Deployment Considerations

### Local Development
- Single binary from `cargo run`
- SQLite database file created automatically
- Frontend served separately or via static file serving

### Production Deployment
- Could deploy backend as a container or binary
- Frontend could be served from CDN or static file host
- SQLite suitable for small-scale deployment; would migrate to PostgreSQL for scale
- Environment variables for database URL and bind address

## Scalability Limitations

- **SQLite**: Not suitable for high-concurrency scenarios
- **No Connection Pooling Config**: Using SQLx defaults
- **Synchronous File Upload**: Large files could block
- **No Caching**: Every request hits the database

## Testing Strategy

- **Unit Tests**: Basic integration test for health endpoint
- **Manual Testing**: Full user flow testing through browser
- **Future**: Would add comprehensive API tests and frontend unit tests

## Performance Characteristics

- **Backend**: Actix-web handles high concurrency efficiently
- **Database**: SQLite fast for read-heavy workloads with low write volume
- **Frontend**: Minimal JavaScript, no framework overhead
- **Auto-save**: Debounced to 2 seconds to reduce API calls

## Error Handling

- **Backend**: Graceful error responses with appropriate HTTP status codes
- **Frontend**: User-friendly error messages and alerts
- **Database**: SQLx provides detailed error information for debugging

## Future Architecture Evolution

If scaling beyond current scope:
1. Migrate from SQLite to PostgreSQL
2. Add Redis caching for frequently accessed documents
3. Implement WebSocket-based real-time collaboration
4. Add authentication layer (JWT or OAuth)
5. Separate frontend build process with bundling
6. Add comprehensive logging and monitoring

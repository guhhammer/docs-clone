# Docs Clone

A lightweight collaborative document editor inspired by Google Docs, built with Rust (Actix-web) backend and vanilla JavaScript frontend.

## Features

- **Document Creation and Editing**: Create, rename, and edit documents with rich text formatting
- **Rich Text Editor**: Support for bold, italic, underline, headings, and lists
- **File Upload**: Import .txt and .md files as new documents
- **Persistence**: Documents are saved to MongoDB
- **User Management**: Simple user ID system for document ownership

## Tech Stack

- **Backend**: Rust with Actix-web framework
- **Database**: MongoDB with mongodb driver
- **Frontend**: Vanilla JavaScript with HTML5 contenteditable
- **Styling**: CSS3

## Prerequisites

- Rust 1.70 or higher
- Cargo (comes with Rust)
- MongoDB (local or remote instance)

## Setup Instructions

1. **Clone the repository**
   ```bash
   git clone <repository-url>
   cd docs-clone
   ```

2. **Install dependencies**
   ```bash
   cargo build
   ```

3. **Configure MongoDB connection**
   Edit `.env` file to set your MongoDB connection string:
   ```
   MONGODB_URL=mongodb://localhost:27017
   ```
   For remote MongoDB:
   ```
   MONGODB_URL=mongodb://username:password@your-server:27017
   ```

4. **Run the backend server**
   ```bash
   cargo run
   ```
   The server will start at `http://127.0.0.1:8080`

5. **Open the frontend**
   Open `frontend/index.html` in your web browser, or serve it with a simple HTTP server:
   ```bash
   cd frontend
   python3 -m http.server 3000
   ```
   Then navigate to `http://localhost:3000`

## Environment Variables

- `MONGODB_URL`: MongoDB connection string (default: `mongodb://localhost:27017`)
- `BIND_ADDRESS`: Server bind address (default: `127.0.0.1:8080`)
- `RUST_LOG`: Logging level (default: `info`)

## API Endpoints

### Health Check
- `GET /` - Health check endpoint

### Documents
- `GET /api/documents?owner_id={user_id}` - Get all documents for a user
- `POST /api/documents` - Create a new document
- `GET /api/documents/{id}` - Get a specific document
- `PUT /api/documents/{id}` - Update a document
- `DELETE /api/documents/{id}` - Delete a document
- `POST /api/documents/upload?owner_id={user_id}` - Upload a file as a new document

## Usage

1. Enter a user ID in the top right corner (default: "user1")
2. Click "+ New Document" to create a new document
3. Click "📁 Upload File" to import a .txt or .md file
4. Use the toolbar to format text (bold, italic, underline, headings, lists)
5. Documents auto-save every 2 seconds, or click "Save" manually
6. Click on documents in the sidebar to open them

## Running Tests

```bash
cargo test
```

## Project Structure

```
docs-clone/
├── src/
│   ├── main.rs          # Application entry point
│   ├── models.rs        # Data models
│   ├── handlers.rs      # HTTP request handlers
│   └── db.rs            # Database operations
├── migrations/
│   └── 001_initial.sql  # Database schema
├── frontend/
│   ├── index.html       # Main HTML file
│   ├── styles.css       # Styling
│   └── app.js           # Frontend logic
├── tests/
│   └── integration_test.rs
└── Cargo.toml           # Rust dependencies
```

## Limitations

- No real-time collaboration
- No authentication system (uses simple user IDs)
- No document versioning
- No export functionality
- File upload limited to .txt and .md files

## Future Enhancements

- Real-time collaboration with WebSockets
- User authentication and authorization
- Document versioning and history
- Export to PDF, Markdown, etc.
- Advanced sharing permissions
- Comments and suggestions mode

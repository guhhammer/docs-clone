# docs-clone

A lightweight collaborative document editor (Google Docs–inspired) built with **Rust + Actix-web** on the backend, **MongoDB** for persistence, and a dependency-free **vanilla JS** rich-text frontend.

Repository: https://github.com/guhhammer/docs-clone
Live demo: https://docsclone.guhhammer.dev/

Everything (API, UI, static assets) is served by a single process on **port 17777**.

---

## Features

- **Documents**: create, rename, edit, autosave, reopen, delete.
- **Rich text**: bold, italic, underline, headings (H1/H2/normal), bulleted and numbered lists, undo/redo. Paste is forced to plain text so foreign markup never enters the document.
- **YouTube embed**: `▶ Video` toolbar button accepts a YouTube URL (watch / `youtu.be` / `embed` / `shorts` / bare video id) and inserts a responsive privacy-mode (`youtube-nocookie.com`) iframe.
- **File upload**: import a **`.txt` or `.md`** file (max **1 MB**) as a new document. Supported types are stated in the UI next to the upload button. Other extensions and non-UTF-8 files are rejected with a clear message.
- **Sharing**: the creator is the owner; owners can grant another user `view` or `edit` access and revoke it. The sidebar separates **My documents** from **Shared with me**, shared cards carry a `SHARED` tag, owned cards show `shared with N`, and the editor shows an access badge (`Owner` / `Shared by X · can edit` / `Shared by X · view only`).
- **Persistence**: MongoDB collections `documents` and `shares`, with indexes on `owner_id + updated_at`, a unique `document_id + user_id` share index, and `user_id`.
- **Security**: every saved title and body is sanitized server-side (see [ARCHITECTURE.md](ARCHITECTURE.md#security-model)).

## Requirements

- Rust (stable, 2021 edition) — `rustc`/`cargo`
- MongoDB running locally (or any reachable MongoDB URL)

## Setup

```bash
git clone https://github.com/guhhammer/docs-clone.git
cd docs-clone
```

Create a `.env` file in the project root:

```bash
MONGODB_URL=mongodb://localhost:27017
BIND_ADDRESS=127.0.0.1:17777
```

Both values have built-in defaults (`mongodb://localhost:27017` and `127.0.0.1:17777`), so `.env` is optional if you use those.

Start MongoDB, then run:

```bash
cargo run --release
```

Open <http://127.0.0.1:17777>.

Health check: `curl http://127.0.0.1:17777/health` → `{"status":"ok"}`

## Tests

```bash
cargo test          # 17 unit tests: sanitizer, injection corpus, policy/CSP guards, validators, handlers
cargo check         # type check
cargo audit         # dependency CVE scan (clean as of the last run)
```

## Reviewing the sharing flow (mocked users)

Authentication is intentionally **mocked**: the identity is whatever user id is typed into the **“Signed in as”** field at the top of the sidebar, sent to the API as an `X-User-Id` header. No passwords, no accounts to provision.

Suggested walkthrough:

1. Open the app, set the user field to `user1`, create a document and type some content.
2. Click **Share**, grant `user2` → `edit`, grant `user3` → `view`.
3. Change the user field to `user2`: the document appears under **Shared with me** and is editable.
4. Change it to `user3`: the same document is read-only — the editor is not editable, the toolbar is disabled, and Save/Share/Delete are hidden.
5. Back as `user1`, reopen the document to see `user2`'s edits, and use **Share** to revoke access.

Any string of letters, digits, `.`, `_`, `-`, `@` (max 64 chars) works as a user id; ids are lowercased.

## API

All endpoints require the `X-User-Id` header (a `?user_id=` query parameter is accepted as a fallback).

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/` | Editor UI |
| `GET` | `/health` | Liveness probe |
| `GET` | `/api/documents` | `{ "owned": [...], "shared": [...] }` |
| `POST` | `/api/documents` | Create (`title`, `content`) |
| `GET` | `/api/documents/{id}` | Fetch one (404 if not visible to the caller) |
| `PUT` | `/api/documents/{id}` | Update title/content (403 for view-only) |
| `DELETE` | `/api/documents/{id}` | Delete + cascade shares (owner only) |
| `POST` | `/api/documents/upload` | Multipart `.txt`/`.md` import |
| `GET` | `/api/documents/{id}/shares` | List shares (owner only) |
| `POST` | `/api/documents/{id}/shares` | Grant/update access (owner only) |
| `DELETE` | `/api/documents/{id}/shares/{user_id}` | Revoke (owner only) |

## Project layout

```
src/
  main.rs             HTTP server, middleware, routes
  compile_config.rs   all tunables and security allow-lists in one place
  security.rs         HTML sanitizer + input validators
  handlers.rs         request handlers, identity + access checks
  db.rs               MongoDB data access
  models.rs           documents, shares, access levels, DTOs
templates/index.html  UI shell
static/               app.js, styles.css
```

## Deployment

Live demo: **<https://docsclone.guhhammer.dev/>** — reviewers can test there without any local setup.

The binary is self-contained (single port, static files served in-process), so the deployment is the same build described above behind a TLS-terminating reverse proxy. See [LAST_THINGS_TODO.md](LAST_THINGS_TODO.md) for the remaining submission steps and [WALKTHROUGH_VIDEO.txt](WALKTHROUGH_VIDEO.txt) for the video link.

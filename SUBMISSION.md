# SUBMISSION

**Project:** docs-clone — a lightweight collaborative document editor (Rust + Actix-web + MongoDB + vanilla JS)
**Author:** guhhammer
**GitHub:** https://github.com/guhhammer/docs-clone
**Live URL:** _not deployed yet — see `LAST_THINGS_TODO.md`_
**Walkthrough video:** see `WALKTHROUGH_VIDEO.txt`
**Google Drive folder:** _to be added when the folder is created — see `LAST_THINGS_TODO.md`_

---

## What is included

| Path | What it is |
| --- | --- |
| `README.md` | Local setup and run instructions, feature list, API reference, sharing-demo script |
| `ARCHITECTURE.md` | Architecture note: priorities, tradeoffs, data model, security model, inspection findings |
| `AI_WORKFLOW.md` | AI workflow note: tools used, where AI helped, what I rejected, how I verified |
| `SUBMISSION.md` | This file |
| `vulnerabilities.txt` | Full hard-to-easy list of known flaws and residual risks |
| `vulnerabilities-patch.txt` | What was fixed from that list (findings 3, 4, 5), why only those three, and the verification output |
| `LAST_THINGS_TODO.md` | Remaining manual submission steps (deploy, video, Drive folder, screenshots) |
| `WALKTHROUGH_VIDEO.txt` | Text file holding the walkthrough video URL |
| `todo.txt` | The original assignment brief, kept for reference |
| `Cargo.toml` / `Cargo.lock` | Rust manifest and pinned dependency graph |
| `src/main.rs` | HTTP server: middleware stack, security headers, routes, port binding |
| `src/compile_config.rs` | Every tunable and security allow-list in one place (paths, port, workers, timeouts, DB names, rate limits, headers, size caps, sanitizer allow-lists) |
| `src/security.rs` | HTML sanitizer + input validators, with unit tests |
| `src/handlers.rs` | Request handlers, mocked identity, access resolution, upload import, with unit tests |
| `src/db.rs` | MongoDB data access and index creation |
| `src/models.rs` | Documents, shares, `Access` levels, response DTOs |
| `templates/index.html` | UI shell: sidebar, editor, share dialog, video dialog |
| `static/app.js` | Editor logic: autosave, formatting, upload, sharing, YouTube embed |
| `static/styles.css` | Styling, responsive layout |
| `.progression/` | Checkpoint logs from earlier AI-assisted sessions |

Everything runs as **one process on port 17777** (API, UI and static assets).

## Credentials / test accounts

**None needed.** Authentication is intentionally mocked: type a user id into the **“Signed in as”** field in the sidebar and the client sends it as an `X-User-Id` header. Use `user1`, `user2` and `user3` to reproduce the sharing flow (full step-by-step script in `README.md` → *Reviewing the sharing flow*). Any id of letters, digits, `.`, `_`, `-`, `@` up to 64 characters works.

## What is working (verified end to end)

- Create, rename, edit, autosave (1.2 s idle), `Ctrl/Cmd+S`, reopen after refresh, delete.
- Rich text: bold, italic, underline, H1/H2/normal, bulleted and numbered lists, undo/redo; paste is coerced to plain text.
- YouTube embed via the `▶ Video` toolbar button (watch / `youtu.be` / `embed` / `shorts` / bare id → responsive `youtube-nocookie.com` iframe); invalid URLs show an inline error.
- File upload: `.txt` and `.md`, max 1 MB, becomes a new editable document; unsupported extensions and non-UTF-8 files are rejected with a clear message. The limit is stated in the UI and in the README.
- Sharing: owner grants `view` or `edit` to another user id and revokes it; sidebar separates **My documents** from **Shared with me**; owned cards show `shared with N`; the editor shows an access badge; view-only users get a read-only editor, a disabled toolbar, and no Save/Share/Delete. Permissions are enforced server-side (view-only `PUT` → 403, non-owner delete/share → 403, invisible document → 404).
- Persistence: MongoDB `documents` + `shares` with indexes; formatting survives the browser → sanitizer → MongoDB → browser round trip.
- Validation and error handling: typed request structs, validated ids/permissions, size caps (1 MiB JSON/upload, 512 KiB content, 200-char titles), rate limiting, generic error responses that do not leak internals.
- Security: server-side HTML sanitization of all stored content, nine hardening headers, no shell/process/filesystem write path, `cargo audit` clean. Details and the full inspection table are in `ARCHITECTURE.md`.
- Concurrent-save safety: saves carry a revision counter; a stale save is rejected with **409** and the UI offers to load the newer version or overwrite it, stashing the local text in the browser first (never silently discarded).
- Tests: `cargo test` → **17 unit tests pass**; `cargo check` clean; API probes with `curl` and a browser walkthrough with headless Playwright (zero console errors).

## What is incomplete

- **No live deployment yet.** The build is testable locally with `cargo run --release`; nothing is hosted at a public URL.
- **No walkthrough video yet.** `WALKTHROUGH_VIDEO.txt` is a placeholder awaiting the link.
- **No Google Drive folder assembled yet.**
- **Auth is mocked.** `X-User-Id` is spoofable and is not a security boundary. Every access check is centralised, but there are no sessions, passwords or CSRF tokens.
- **No screenshots / demo GIF committed.**
- **Not real-time.** There is no live cursor/presence indicator and no merge: concurrent edits are detected (revision conflict, 409, explicit choice in the UI), not merged.
- **No comments, version history, PDF export, or `.docx` import.**
- **Share targets are unvalidated identities** — you can share with a user id that nobody has ever used; it simply appears for whoever types that id.
- Uploads are limited to plain text formats, and no attachment storage exists (imported files become documents, they are not kept as files).
- Document lists are unpaginated.

## What I would build next with another 2–4 hours

1. **Deploy** behind the existing reverse proxy with TLS and an authenticated MongoDB URL, then record the walkthrough video and assemble the Drive folder (~45 min).
2. **Replace mocked auth with real sessions** — signed cookie, a `users` collection, login form, CSRF token. `resolve_access` already isolates the identity source, so this is contained (~1 h).
3. **Presence and merge on top of the existing conflict detection** — show who else has the document open, and offer a diff instead of the current overwrite-or-reload choice (~40 min).
4. **Share-link + role polish** — invite by link, `comment` role, and an owner-visible activity line (“shared with user2 · edit · 3 days ago”) (~40 min).
5. **Integration tests against a throwaway MongoDB** covering the access matrix (owner/edit/view/stranger × read/write/delete/share) so the permission logic is regression-proof (~30 min).

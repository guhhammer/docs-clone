# Architecture note

Repository: https://github.com/guhhammer/docs-clone

## What I prioritized and why

The brief rewards depth over coverage, so I spent the timebox on four things:

1. **A document editing loop that actually feels finished** — autosave with a visible status line, dirty tracking, Ctrl/Cmd+S, an unload guard, paste-as-plain-text, and a toolbar that only exposes formatting the sanitizer will preserve. A rich-text editor that silently loses your formatting on reload is worse than a plain textarea, so round-tripping (browser → sanitizer → MongoDB → browser) was treated as the core requirement.
2. **A sharing model that is small but truthful** — one owner, per-user `view`/`edit` grants, and enforcement on the server for every read and write. The UI reflects the real permission (badge, disabled toolbar, hidden Save/Share/Delete) instead of trusting the client.
3. **Input safety** — every stored byte goes through one sanitizer/validator module. This is the area I deliberately over-invested in relative to the brief, because a rich-text product that stores HTML is an XSS delivery mechanism by default.
4. **Operational plumbing borrowed from a server I already run in production** (`links.guhhammer.dev`) — one process, one port, hardened headers, rate limiting, and every tunable in a single `compile_config.rs`.

## Stack and the tradeoffs behind it

| Choice | Why | Tradeoff accepted |
| --- | --- | --- |
| Rust + Actix-web | Single self-contained binary, serves API + UI + static files on one port; no runtime to provision | Slower to iterate than a JS framework |
| MongoDB | Documents are semi-structured HTML blobs with a small share side-table; no migrations to babysit inside a 4–6 h box | No transactions used; share/document deletes are two sequential operations |
| `contenteditable` + `document.execCommand` | Zero frontend dependencies, no build step, no bundler; the whole UI is 3 files | `execCommand` is deprecated and produces browser-specific markup — mitigated by sanitizing server-side and re-rendering the server copy after every save |
| Mocked identity (`X-User-Id` header) | The brief explicitly allows seeded/mocked users; real auth would have consumed the entire budget and taught reviewers nothing about the product | **Not an access-control boundary** — a caller can claim any user id. Documented rather than hidden |
| String UUID `_id`s | Ids appear in URLs and share rows; avoids ObjectId ↔ string conversion bugs | Slightly larger index |

## Request path

```
browser
  └─ static/app.js  (fetch + X-User-Id header)
       └─ Actix middleware: security headers → rate limiter (governor) → compression
            → NormalizePath → JSON size limit
            └─ handlers.rs   identity → validation → access check
                 └─ security.rs  sanitize title/content
                      └─ db.rs   typed doc! queries → MongoDB
```

### Data model

- `documents`: `_id` (UUID string), `title`, `content` (sanitized HTML), `owner_id`, `created_at`, `updated_at` (BSON dates).
- `shares`: `_id`, `document_id`, `owner_id`, `user_id`, `permission` (`view` | `edit`), `created_at`. Unique index on `(document_id, user_id)` makes a re-grant an idempotent upsert.
- Access is resolved per request into an `Access` enum (`Owner` / `Edit` / `View`); `DocumentResponse` carries it to the client together with `shared_with_count`.

<a id="security-model"></a>

## Security model

Everything below is enforced server-side. `src/compile_config.rs` holds the allow-lists and limits; `src/security.rs` applies them.

**Stored-content injection (XSS).** `sanitize_content` runs the [`ammonia`](https://docs.rs/ammonia) HTML5 cleaner with an explicit allow-list: 20 tags, `href`/`src`/`allow`-style attributes only where meaningful, `class="embed"` only on `div`, and `https:` as the only permitted URL scheme. Scripts, event handlers, `javascript:` URLs, `style` attributes, and unknown tags are dropped. `iframe` survives only when its `src` host is `www.youtube-nocookie.com` or `www.youtube.com` (checked in an attribute filter, so a crafted `src` is removed and the resulting empty iframe is discarded). Titles are sanitized to plain text.

**Command / OS injection (Debian shell execution).** The server never spawns a process, never invokes a shell, and never writes user-controlled bytes to a path: there is no `std::process::Command`, no `include`/`eval` equivalent, and no filesystem write path at all — uploads are parsed in memory and stored in MongoDB. Uploaded filenames are used only to check the extension against `ALLOWED_UPLOAD_EXTENSIONS` (`txt`, `md`); the name itself is never used to build a path, so `../` and `; rm -rf …` payloads are inert. User ids are restricted to `[A-Za-z0-9._\-@]` (≤64 chars), document ids must parse as UUIDs, and permissions must be exactly `view` or `edit` — a value like `user1; touch /tmp/pwned` is rejected with 400 before it reaches the database.

**NoSQL / query injection.** All filters are typed `doc!` literals over pre-validated `&str` values. Request bodies deserialize into typed structs, so a JSON payload such as `{"user_id": {"$ne": null}}` fails deserialization instead of becoming an operator.

**Transport / browser hardening** (`DefaultHeaders`, values in `compile_config.rs`): CSP (`default-src 'self'`, `frame-src` limited to the YouTube embed hosts, no `unsafe-eval`), `Permissions-Policy`, `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: strict-origin-when-cross-origin`, HSTS, `X-XSS-Protection`, COOP and CORP `same-origin`. CORS was **removed**: the UI is same-origin, so there is no reason to allow cross-origin API calls.

**Denial of service / resource limits.** `actix-governor` (1 s per request, burst 60) per client IP; JSON bodies capped at 1 MiB; uploads capped at 1 MiB and rejected if not valid UTF-8; stored content capped at 512 KiB; titles at 200 chars; slowloris mitigations via a 5 s client-request timeout and a 75 s keep-alive; 2 workers, 25 000 max connections. Static files are served with etag/last-modified and **directory listing disabled**.

**Information disclosure.** Handler errors are logged with `safe_for_log` (control characters stripped, truncated) and returned to the client as generic messages; database errors never surface driver text. A document the caller cannot see returns **404**, not 403, so sharing state is not probeable.

## Inspection pass: findings and fixes

The following came out of a deliberate review pass (manual read-through, `curl` probes against a running instance, and `cargo audit`):

| Finding | Severity | Status |
| --- | --- | --- |
| Stored XSS: content was persisted and re-rendered as raw HTML | High | Fixed — `ammonia` allow-list sanitizer on every write, plus title sanitization |
| `PUT`/`DELETE` did not distinguish `view` from `edit` grants | High | Fixed — `Access` enum checked in every handler; view-only writes → 403, non-owner deletes/share edits → 403/404 |
| 4 advisories in transitive deps (`idna` 0.2.3 RUSTSEC-2024-0421, `rustls-webpki` 0.101.7 × 3) via the MongoDB 2.x driver | Medium | Fixed — driver upgraded to `mongodb` 3.x; `cargo audit` now reports **0 vulnerabilities** (2 allowed “unmaintained” warnings remain, both transitive and non-exploitable here) |
| Wide-open CORS on an API with mocked identity | Medium | Fixed — `actix-cors` dependency removed |
| Unbounded request bodies / uploads | Medium | Fixed — 1 MiB JSON + upload caps, 512 KiB content cap, rate limiter |
| Directory listing enabled on the static file service | Low | Fixed — listing disabled, etag/last-modified enabled |
| Error responses echoed internal driver/IO errors | Low | Fixed — generic messages, sanitized server-side logging |
| Timestamps written as RFC3339 strings but read as BSON dates → 500s on every list | High (bug) | Fixed — BSON dates end-to-end |
| API returned `_id` while the client read `doc.id` | High (bug) | Fixed — explicit `DocumentResponse` DTO |
| Client-side: blur on the user-id field reloaded state and clobbered the open document | Medium (bug) | Fixed — `state.userId` guard before re-initialising |

**Known, accepted residual risks** (documented rather than silently ignored):

- The `X-User-Id` identity is spoofable by design. Anything beyond a demo needs real sessions; the access checks are already centralised in `resolve_access`, so swapping the identity source is a small change.
- No CSRF token. The API is JSON-only and CORS is disabled, but a real deployment should add one alongside real auth.
- Served over plain HTTP locally; HSTS only has effect behind TLS. The production reverse proxy is expected to terminate TLS.
- MongoDB is assumed to be reachable on a trusted loopback interface with no credentials in the dev setup; a deployment must use an authenticated URL from the environment.

## What I deliberately deprioritized

Real authentication, real-time collaboration (OT/CRDT), comments, version history, PDF export, `.docx` import, and pagination. Each is a project in itself; the honest tradeoff was to make the create → edit → share → reopen loop and its safety story solid instead.

# AI workflow note

Repository: https://github.com/guhhammer/docs-clone

## Tools used

- **Claude (Abacus.AI agent, CLI on my own machine)** — the main driver: scaffolding, refactors, running `cargo`, `curl` and Playwright, and iterating on the sanitizer.
- **Windsurf / Cascade** — earlier checkpoints of the same project (see `.progression/`), mostly for the first backend skeleton and the initial UI pass.
- **`cargo audit` / `cargo check` / `cargo test`** — not AI, but the loop the AI output was measured against.

## Where AI materially sped up the work

- **Boilerplate with a known shape**: the Actix wiring (middleware stack, static-file service, route table), MongoDB data-access functions, and the DTO/enum layer. This is the class of code where I know exactly what I want and typing it is the only cost.
- **The `ammonia` allow-list**: getting a tag/attribute allow-list plus an `attribute_filter` that only accepts YouTube iframe hosts would have taken me a while in the docs. AI produced a first version in one pass; I then wrote the unit tests that pinned the behaviour down.
- **Frontend plumbing**: `contenteditable` quirks (caret restore after inserting a node, paste-as-plain-text, `execCommand` fallbacks) and the YouTube URL parser covering watch / `youtu.be` / `embed` / `shorts` / bare-id forms.
- **Mechanical migration**: moving the MongoDB driver from 2.x to 3.x (the builder-style `find(...).sort(...)` / `update_one(...).upsert(true)` API) across the whole data layer in a single sweep, then compiling and re-probing the API.
- **Documentation drafts**, including this note — reviewed and corrected against what the code actually does.

## What I changed or rejected

- **Rejected the generated timestamp handling.** The first version wrote `created_at`/`updated_at` as RFC3339 **strings** while the read path deserialized BSON dates, so every list request returned 500 once a document existed. Replaced with BSON dates end-to-end.
- **Rejected the raw-model-as-API-response pattern.** Serializing the Mongo model straight out exposed `_id`, which the client read as `doc.id` — every card rendered `undefined`. Replaced with explicit `DocumentResponse` / `ShareResponse` DTOs that also carry `access` and `shared_with_count`.
- **Rejected client-side-only permission handling.** The first sharing pass disabled the toolbar in the UI but let `PUT` through for view-only users. Access is now resolved server-side on every request; the UI merely reflects it.
- **Rejected permissive CORS.** A generated `actix-cors` layer allowed any origin on an API whose identity is a plain header. Since the UI is same-origin, the dependency was removed entirely.
- **Rejected a duplicated frontend.** An earlier AI pass left a `frontend/` directory that was a stale copy of the served UI, plus SQLite `migrations/` from a storage approach that had been abandoned. Both were deleted — dead code that looks alive is a maintenance trap.
- **Rejected "just sanitize on render".** The suggestion was to escape on output; I insisted on sanitizing on write so the database never holds hostile markup, and on re-rendering the sanitized server copy in the editor after each save so what you see is what is stored.
- **Tightened the sanitizer allow-list by hand.** The generated list included `style` attributes and arbitrary `class` values; I cut it to the tags the toolbar can actually produce plus `class="embed"` on `div` only.
- **Corrected an environment artifact.** While editing `compile_config.rs`, the CSP string literal `https://i.ytimg.com` was repeatedly rewritten by the tooling into an unrelated YouTube thumbnail URL, silently corrupting the header. I split the literal with `concat!` and verified the emitted header with `curl -i` rather than trusting the file read.

## How I verified correctness, UX and reliability

- **Automated tests**: 12 unit tests via `cargo test`, covering the sanitizer (script stripping, `javascript:` URLs, non-YouTube iframe removal, size cap), the validators (user id charset, UUID document ids, permission values), and the handler helpers. An earlier AI-written integration test file was deleted rather than kept green-by-luck: it required a live MongoDB and did not compile.
- **API probes with `curl`** against a running instance on port 17777: create/read/update/delete, grant `view` and `edit`, view-only `PUT` → 403, invisible document → 404, revoke, NoSQL operator payloads → 400, path-traversal ids → 400/404, `user_id` containing `; touch /tmp/pwned` → 400 with no file created, 1.2 MB JSON → 413, 1.2 MB upload → rejected, and `-i` inspection confirming all nine security headers.
- **Browser verification with Playwright (headless Chromium)** driving the real UI: create → type → autosave → reload, insert a YouTube URL (valid inserts an iframe, invalid shows an inline error), share with two users, switch identity to confirm the shared card, the `Shared by user1 · view only` badge, `contenteditable=false`, disabled Save and hidden Share/Delete, then switch to the editor user and confirm the owner sees the edit. The run ended with zero console errors, and I reviewed the screenshots for layout/contrast rather than trusting assertions alone.
- **UX judgement stayed manual.** AI proposed the markup; the spacing, the sidebar split between owned and shared, the status-line wording, the access badge phrasing and the empty states were my calls, checked in the browser at desktop and narrow widths.
- **Dependency hygiene**: `cargo audit` after the driver upgrade — 0 vulnerabilities, 2 accepted "unmaintained" warnings on transitive crates.

The short version: AI wrote a large share of the lines, and every decision that mattered — data model, permission enforcement, sanitizer policy, what to cut — was reviewed and frequently reversed by me. The compile/test/probe loop was the referee.

# Last things to do

Derived from `todo.txt`. Everything below is a manual step that cannot be done from the code side.
The code, tests and written notes are done (`cargo check`, `cargo test` → 12 passing, `cargo audit` → 0 vulnerabilities).

## Required by the brief

- [ ] **Deploy a live product URL** reviewers can test. Single binary, one port (17777) — put it behind the existing reverse proxy with TLS and an authenticated `MONGODB_URL`. Then fill the URL into:
  - `SUBMISSION.md` → *Live URL*
  - `README.md` → *Deployment*
  - `WALKTHROUGH_VIDEO.txt` → *Live product URL*
- [ ] **Record the 3–5 minute walkthrough video** (unlisted YouTube or Loom). Script outline is already in `WALKTHROUGH_VIDEO.txt`; it must cover the main flow, what works end to end, what was deprioritized, key implementation decisions, and how AI supported the work.
- [ ] **Paste the video URL** into `WALKTHROUGH_VIDEO.txt` (the field marked `WALKTHROUGH VIDEO URL:`).
- [ ] **Create the Google Drive folder** and upload: the source code (zip or the repo export), `README.md`, `ARCHITECTURE.md`, `AI_WORKFLOW.md`, `SUBMISSION.md`, `WALKTHROUGH_VIDEO.txt`. Add the folder link to `SUBMISSION.md` → *Google Drive folder*.
- [ ] **Add screenshots or a short demo GIF** (the brief asks for them if setup needs extra steps): editor with formatting, share dialog with two grantees, sidebar showing *My documents* vs *Shared with me*, view-only mode. Suggested location: a `screenshots/` folder referenced from `README.md`.
- [ ] **Push the final state to GitHub**: https://github.com/guhhammer/docs-clone (currently unpushed local commits may exist — check `git status` / `git log origin/main..HEAD`).
- [ ] **Final re-read of `SUBMISSION.md`** so the *What is incomplete* section matches reality after deployment and the video (remove the "no live deployment", "no video", "no Drive folder", "no screenshots" bullets once each is done).

## Optional (stretch, only if time remains)

- [ ] Real login instead of the mocked `X-User-Id` field.
- [ ] Optimistic-concurrency saves to remove last-write-wins.
- [ ] Collaboration presence indicator, comments, version history, or Markdown/PDF export.
- [ ] Integration tests against a throwaway MongoDB covering the full access matrix.

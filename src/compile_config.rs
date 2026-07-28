//! Centralized compile-time and runtime default constants.
//!
//! Mirrors the hardening baseline used by the production `links.guhhammer.dev`
//! server (security headers, rate limiting, slowloris protections) and adds the
//! limits that are specific to a document editor (payload sizes, upload rules,
//! sanitizer allow-lists).

// ---------------------------------------------------------------------------
// Filesystem / assets
// ---------------------------------------------------------------------------

/// Filesystem directory containing static assets served by Actix.
pub const STATIC_DIR_PATH: &str = "./static";
/// Glob pattern for Tera templates.
pub const TEMPLATES_GLOB: &str = "templates/**/*";
/// Template name for the index page.
pub const TEMPLATE_INDEX: &str = "index.html";
/// URL prefix for static assets.
pub const STATIC_ROUTE: &str = "/static";

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// Host/interface to bind the HTTP server on.
pub const SERVER_BIND_HOST: &str = "127.0.0.1";
/// Port to bind the HTTP server on. The whole app (API + UI) runs here.
pub const SERVER_BIND_PORT: u16 = 17777;
/// Number of Actix workers.
pub const SERVER_WORKERS: usize = 2;
/// Maximum concurrent connections (slowloris protection).
pub const SERVER_MAX_CONNECTIONS: usize = 25_000;
/// Client request timeout (slowloris protection).
pub const CLIENT_REQUEST_TIMEOUT_SECS: u64 = 5;
/// Keep-alive duration for HTTP connections.
pub const KEEP_ALIVE_SECS: u64 = 75;

// ---------------------------------------------------------------------------
// Database
// ---------------------------------------------------------------------------

/// Default MongoDB connection string when `MONGODB_URL` is not set.
pub const DEFAULT_MONGODB_URL: &str = "mongodb://localhost:27017";
/// Logical database name.
pub const DB_NAME: &str = "docs_clone";
/// Collection holding documents.
pub const COLLECTION_DOCUMENTS: &str = "documents";
/// Collection holding document shares.
pub const COLLECTION_SHARES: &str = "shares";
/// Server selection timeout so a dead database fails fast instead of hanging.
pub const DB_SERVER_SELECTION_TIMEOUT_SECS: u64 = 5;

// ---------------------------------------------------------------------------
// Rate limiting (actix-governor)
// ---------------------------------------------------------------------------

/// Governor rate limit window in seconds (token refill interval).
pub const RATE_LIMIT_SECONDS_PER_REQUEST: u64 = 1;
/// Governor burst size.
pub const RATE_LIMIT_BURST_SIZE: u32 = 60;

// ---------------------------------------------------------------------------
// Security headers
// ---------------------------------------------------------------------------

/// Content-Security-Policy. `frame-src` is limited to YouTube so embedded
/// videos work without opening the page to arbitrary third-party frames.
pub const HEADER_CSP: (&str, &str) = (
    "Content-Security-Policy",
    concat!(
        "default-src 'self'; ",
        "base-uri 'none'; ",
        "object-src 'none'; ",
        "script-src 'self'; ",
        "style-src 'self'; ",
        "img-src 'self' data: https://", "i.ytimg.com", "; ",
        "frame-src https://", "www.youtube-nocookie.com", " https://", "www.youtube.com", "; ",
        "frame-ancestors 'none'; ",
        "form-action 'none'; ",
        "connect-src 'self'"
    ),
);
/// Permissions-Policy header key/value.
pub const HEADER_PERMISSIONS_POLICY: (&str, &str) = (
    "Permissions-Policy",
    "geolocation=(), camera=(), microphone=(), payment=()",
);
/// X-XSS-Protection header key/value.
pub const HEADER_XXS_PROTECTION: (&str, &str) = ("X-XSS-Protection", "1; mode=block");
/// X-Frame-Options header key/value.
pub const HEADER_X_FRAME_OPTIONS: (&str, &str) = ("X-Frame-Options", "DENY");
/// X-Content-Type-Options header key/value.
pub const HEADER_X_CONTENT_TYPE_OPTIONS: (&str, &str) = ("X-Content-Type-Options", "nosniff");
/// Referrer-Policy header key/value.
pub const HEADER_REFERRER_POLICY: (&str, &str) =
    ("Referrer-Policy", "strict-origin-when-cross-origin");
/// Strict-Transport-Security header key/value.
pub const HEADER_HSTS: (&str, &str) = (
    "Strict-Transport-Security",
    "max-age=31536000; includeSubDomains",
);
/// Cross-Origin-Opener-Policy header key/value.
pub const HEADER_COOP: (&str, &str) = ("Cross-Origin-Opener-Policy", "same-origin");
/// Cross-Origin-Resource-Policy header key/value.
pub const HEADER_CORP: (&str, &str) = ("Cross-Origin-Resource-Policy", "same-origin");

// ---------------------------------------------------------------------------
// Input limits (injection / abuse surface reduction)
// ---------------------------------------------------------------------------

/// Maximum accepted JSON body size (1 MiB).
pub const MAX_JSON_PAYLOAD_BYTES: usize = 1024 * 1024;
/// Maximum accepted uploaded file size (1 MiB).
pub const MAX_UPLOAD_BYTES: usize = 1024 * 1024;
/// Maximum stored document content length in bytes, measured after sanitizing.
pub const MAX_CONTENT_BYTES: usize = 512 * 1024;
/// Maximum document title length in characters.
pub const MAX_TITLE_CHARS: usize = 200;
/// Maximum user id length in characters.
pub const MAX_USER_ID_CHARS: usize = 64;
/// Title used when the client sends an empty/blank title.
pub const DEFAULT_TITLE: &str = "Untitled document";

/// File extensions accepted by the import endpoint.
pub const ALLOWED_UPLOAD_EXTENSIONS: [&str; 2] = ["txt", "md"];

// ---------------------------------------------------------------------------
// HTML sanitizer allow-list
// ---------------------------------------------------------------------------

/// HTML tags preserved when sanitizing document content.
pub const SANITIZER_ALLOWED_TAGS: [&str; 20] = [
    "p", "br", "b", "strong", "i", "em", "u", "s", "h1", "h2", "h3", "ul", "ol", "li", "blockquote",
    "code", "pre", "span", "div", "iframe",
];

/// Tag/attribute pairs preserved when sanitizing document content.
/// `iframe` is allowed only for YouTube embeds, restricted by URL host below.
pub const SANITIZER_ALLOWED_ATTRIBUTES: [(&str, &[&str]); 1] = [(
    "iframe",
    &[
        "src",
        "width",
        "height",
        "title",
        "allow",
        "allowfullscreen",
        "frameborder",
        "loading",
        "referrerpolicy",
    ],
)];

/// CSS classes preserved per tag. Only the responsive embed wrapper is allowed.
pub const SANITIZER_ALLOWED_CLASSES: [(&str, &[&str]); 1] = [("div", &["embed"])];

/// URL hosts allowed as `iframe` sources (YouTube embeds only).
pub const SANITIZER_ALLOWED_IFRAME_HOSTS: [&str; 2] = ["www.youtube-nocookie.com", "www.youtube.com"];

/// URL schemes allowed anywhere in document content.
pub const SANITIZER_ALLOWED_URL_SCHEMES: [&str; 1] = ["https"];
use std::path::{Path, PathBuf};
use std::sync::Arc;
use axum::{
    Router,
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::get,
};
use serde::Serialize;
use tokio::sync::RwLock;

use crate::daemon::engine::{ProcessManager, ServiceStatus};
use crate::firewall::panic::{PanicEngine, PanicStatus};

#[derive(Clone)]
pub struct HttpApiState {
    pub process_manager: Arc<RwLock<ProcessManager>>,
    pub panic_engine: Arc<RwLock<PanicEngine>>,
    pub ui_token: String,
    pub http_port: u16,
    pub first_run: bool,
    pub setup_password: Option<String>,
}

#[derive(Serialize)]
struct StatusResponse {
    services: Vec<ServiceInfoJson>,
    panic: PanicStatus,
    first_run: bool,
}

#[derive(Serialize)]
struct ServiceInfoJson {
    name: String,
    status: String,
    uptime_secs: u64,
    restart_count: u32,
    pid: Option<u32>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn service_status_str(s: &ServiceStatus) -> &str {
    match s {
        ServiceStatus::Stopped => "stopped",
        ServiceStatus::Starting => "starting",
        ServiceStatus::Running => "running",
        ServiceStatus::Stopping => "stopping",
        ServiceStatus::Failed(_) => "failed",
        ServiceStatus::Restarting => "restarting",
    }
}

const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; font-src 'self'";

fn is_local_origin(origin: &str) -> bool {
    origin == "http://localhost"
        || origin.starts_with("http://localhost:")
        || origin == "http://127.0.0.1"
        || origin.starts_with("http://127.0.0.1:")
}

fn check_origin(request: &Request) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if let Some(origin) = request.headers().get("Origin").and_then(|v| v.to_str().ok()) {
        if !is_local_origin(origin) {
            return Err((StatusCode::FORBIDDEN, Json(ErrorResponse {
                error: "Cross-origin requests denied".into(),
            })));
        }
    }
    Ok(())
}

fn mime_for_path(path: &str) -> &str {
    if path.ends_with(".html") { "text/html" }
    else if path.ends_with(".css") { "text/css" }
    else if path.ends_with(".js") { "application/javascript" }
    else if path.ends_with(".svg") { "image/svg+xml" }
    else if path.ends_with(".png") { "image/png" }
    else if path.ends_with(".ico") { "image/x-icon" }
    else if path.ends_with(".woff2") { "font/woff2" }
    else if path.ends_with(".json") { "application/json" }
    else { "application/octet-stream" }
}

fn inject_token_into_html(content: &str, token: &str, first_run: bool, setup_password: Option<&str>) -> String {
    let mut meta = format!(r#"<meta name="api-token" content="{token}">"#);
    if first_run {
        meta.push_str(r#"<meta name="first-run" content="true">"#);
        if let Some(pw) = setup_password {
            meta.push_str(&format!(r#"<meta name="setup-password" content="{pw}">"#));
        }
    }
    content.replace("</head>", &format!("{meta}</head>"))
}

async fn auth_middleware(
    State(state): State<HttpApiState>,
    request: Request,
    next: Next,
) -> Response {
    if let Some(origin) = request.headers().get("Origin").and_then(|v| v.to_str().ok()) {
        if !is_local_origin(origin) {
            return (StatusCode::FORBIDDEN, Json(ErrorResponse {
                error: "Cross-origin requests denied".into(),
            })).into_response();
        }
    }

    let token = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");

    if token != state.ui_token {
        return (StatusCode::UNAUTHORIZED, Json(ErrorResponse {
            error: "Invalid or missing authentication token".into(),
        })).into_response();
    }

    next.run(request).await
}

async fn handle_get_status(
    State(state): State<HttpApiState>,
) -> Json<StatusResponse> {
    let pm = state.process_manager.read().await;
    let services = pm.all_status().await;
    let pe = state.panic_engine.read().await;
    let panic = pe.status().await;

    Json(StatusResponse {
        services: services
            .into_iter()
            .map(|s| ServiceInfoJson {
                name: s.name.display_name().to_string(),
                status: service_status_str(&s.status).to_string(),
                uptime_secs: s.uptime_secs,
                restart_count: s.restart_count,
                pid: s.pid,
            })
            .collect(),
        panic,
        first_run: state.first_run,
    })
}

async fn handle_get_panic(
    State(state): State<HttpApiState>,
) -> Json<PanicStatus> {
    let pe = state.panic_engine.read().await;
    Json(pe.status().await)
}

fn dist_dir() -> PathBuf {
    // Check known install locations first
    for candidate in &[
        "/opt/kryptos/dist",
        "/usr/local/share/kryptos/dist",
    ] {
        let p = PathBuf::from(candidate);
        if p.join("index.html").exists() {
            return p;
        }
    }
    // Walk up from binary looking for dist/index.html
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().unwrap_or(Path::new(".")).to_path_buf();
        loop {
            let candidate = dir.join("dist");
            if candidate.join("index.html").exists() {
                return candidate;
            }
            if !dir.pop() {
                break;
            }
        }
    }
    PathBuf::from("./dist")
}

use std::path::Path;

async fn handle_index(
    State(state): State<HttpApiState>,
    request: Request,
) -> Result<Response, (StatusCode, String)> {
    if let Err((code, json)) = check_origin(&request) {
        return Err((code, json.error.clone()));
    }
    let content = tokio::fs::read_to_string(dist_dir().join("index.html"))
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "index.html not found".into()))?;

    let injected = inject_token_into_html(&content, &state.ui_token, state.first_run, state.setup_password.as_deref());
    let mut resp = ([(header::CONTENT_TYPE, "text/html")], injected).into_response();
    resp.headers_mut().insert(header::CONTENT_SECURITY_POLICY, CSP.parse().unwrap());
    Ok(resp)
}

/// SPA fallback: serve static files from dist_dir, or fall back to index.html with token injected
async fn handle_spa_fallback(
    State(state): State<HttpApiState>,
    request: Request,
) -> Response {
    // V-15: Check Origin on SPA fallback to prevent cross-origin token extraction
    if let Some(origin) = request.headers().get("Origin").and_then(|v| v.to_str().ok()) {
        if !is_local_origin(origin) {
            return (StatusCode::FORBIDDEN, Json(ErrorResponse {
                error: "Cross-origin requests denied".into(),
            })).into_response();
        }
    }

    let path = dist_dir().join(request.uri().path().strip_prefix('/').unwrap_or(""));

    if let Ok(content) = tokio::fs::read(&path).await {
        let mime = mime_for_path(path.to_str().unwrap_or(""));
        return ([(header::CONTENT_TYPE, mime)], content).into_response();
    }

    // SPA fallback: serve index.html with token injection
    match tokio::fs::read_to_string(dist_dir().join("index.html")).await {
        Ok(content) => {
            let injected = inject_token_into_html(&content, &state.ui_token, state.first_run, state.setup_password.as_deref());
            let mut resp = ([(header::CONTENT_TYPE, "text/html")], injected).into_response();
            resp.headers_mut().insert(header::CONTENT_SECURITY_POLICY, CSP.parse().unwrap());
            resp
        }
        Err(_) => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}

pub fn create_router(state: HttpApiState) -> Router {
    let api_routes = Router::new()
        .route("/api/status", get(handle_get_status))
        .route("/api/panic", get(handle_get_panic))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .route("/", get(handle_index))
        .route("/index.html", get(handle_index))
        .merge(api_routes)
        .fallback(handle_spa_fallback)
        .with_state(state)
}

pub async fn run_http_server(
    state: HttpApiState,
) -> anyhow::Result<()> {
    let bind_addr = std::net::Ipv4Addr::new(127, 0, 0, 1);
    let addr = std::net::SocketAddrV4::new(bind_addr, state.http_port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let router = create_router(state);
    tracing::info!("Web UI available at http://{}", listener.local_addr()?);
    axum::serve(listener, router).await?;
    Ok(())
}

use crate::{
    app_container::AppContainer,
    error::CoreError,
    types::{ActiveSlideInfo, DeckParserProfile, NoteInput, TakeGroupResponse},
};
use axum::{
    body::{Body, Bytes},
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        DefaultBodyLimit, Path, State,
    },
    http::{header, HeaderMap, HeaderValue, Method, Response, StatusCode},
    response::{IntoResponse, Response as AxumResponse},
    routing::{get, patch, post, put},
    Json, Router,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::{net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, sync::broadcast, task};
use tokio_util::io::ReaderStream;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

const MAX_AUDIO_UPLOAD_BYTES: usize = 512 * 1024 * 1024;
const BRIDGE_SDK: &str = include_str!("../../saist-web/public/sai-slidetake-bridge.js");
const EMBEDDED_UI_SDK: &str = include_str!("../../saist-web/public/sai-slidetake-embedded.js");
const HTML2CANVAS_SDK: &str =
    include_str!("../../saist-web/public/vendor/html2canvas.min.js");

#[derive(Debug)]
struct ApiError(CoreError);

impl From<CoreError> for ApiError {
    fn from(value: CoreError) -> Self {
        Self(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> AxumResponse {
        let status = match &self.0 {
            CoreError::Invalid(_) => StatusCode::BAD_REQUEST,
            CoreError::NotFound(_) => StatusCode::NOT_FOUND,
            CoreError::Conflict(_) => StatusCode::CONFLICT,
            CoreError::Io(_) | CoreError::Json(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(json!({"error": self.0.to_string()}))).into_response()
    }
}

pub fn router(container: Arc<AppContainer>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _| is_local_origin(origin)))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any)
        .expose_headers([header::CONTENT_LENGTH, header::CONTENT_TYPE]);

    Router::new()
        .route("/api/slide/active", post(set_active_slide))
        .route(
            "/api/slides/{slide_id}/takes",
            get(get_slide_takes).post(save_slide_take),
        )
        .route("/api/slides/{slide_id}/golden", put(set_golden_take))
        .route(
            "/api/slides/{slide_id}/takes/{take_id}",
            patch(update_take_note).delete(delete_slide_take),
        )
        .route("/api/manifest", get(get_manifest))
        .route("/api/recording/start", post(start_recording))
        .route("/api/recording/stop", post(stop_recording))
        .route("/api/recording/status", get(recording_status))
        .route("/api/deck", get(get_deck))
        .route("/api/deck/ingest", post(ingest_deck))
        .route("/api/deck/test-parser", post(test_parser_profile))
        .route("/api/deck/apply-parser", post(apply_parser_profile))
        .route("/api/deck/notes/{slide_id}", get(get_slide_notes))
        .route("/api/deck/export-master-audio", post(export_master_audio))
        .route("/api/deck/master-audio/{filename}", get(get_master_audio))
        .route("/api/deck/export-video", post(start_video_export))
        .route(
            "/api/deck/video-export/{job_id}",
            get(get_video_export_status),
        )
        .route(
            "/api/deck/video-export/{job_id}/finish",
            post(finish_video_export),
        )
        .route(
            "/api/deck/video-export/{job_id}/fail",
            post(fail_video_export),
        )
        .route(
            "/api/deck/video-export/{job_id}/slides/{slide_index}",
            put(upload_video_slide),
        )
        .route("/api/deck/video/{filename}", get(get_video))
        .route("/api/audio/{slide_id}/{filename}", get(get_audio))
        .route("/sai-slidetake-bridge.js", get(get_bridge_sdk))
        .route("/sai-slidetake-embedded.js", get(get_embedded_ui_sdk))
        .route("/sai-slidetake-html2canvas.js", get(get_html2canvas_sdk))
        .route("/ws", get(websocket_upgrade))
        .layer(DefaultBodyLimit::max(MAX_AUDIO_UPLOAD_BYTES))
        .layer(cors)
        .with_state(container)
}

fn is_local_origin(origin: &HeaderValue) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    if origin == "null" || origin == "tauri://localhost" {
        return true;
    }
    let Some((scheme, authority)) = origin.split_once("://") else {
        return false;
    };
    if !matches!(scheme, "http" | "https") || authority.contains('/') {
        return false;
    }
    let host = authority.split(':').next().unwrap_or_default();
    matches!(host, "localhost" | "127.0.0.1" | "tauri.localhost")
}

pub async fn serve(address: SocketAddr, container: Arc<AppContainer>) -> std::io::Result<()> {
    let listener = TcpListener::bind(address).await?;
    println!("[SAISlideTake] WebAPI + WebSocket listening on {address}");
    axum::serve(listener, router(container)).await
}

async fn set_active_slide(
    State(container): State<Arc<AppContainer>>,
    Json(payload): Json<ActiveSlideInfo>,
) -> Result<Json<Value>, ApiError> {
    let app = container.clone();
    let active_slide = payload.clone();
    task::spawn_blocking(move || app.set_active_slide(active_slide))
        .await
        .map_err(|error| CoreError::Conflict(format!("更新目前投影片失敗：{error}")))??;
    Ok(Json(json!({"ok": true, "active_slide": payload})))
}

async fn get_slide_takes(
    State(container): State<Arc<AppContainer>>,
    Path(slide_id): Path<String>,
) -> Result<Json<TakeGroupResponse>, ApiError> {
    Ok(Json(container.slide_takes(&slide_id)?.into()))
}

async fn save_slide_take(
    State(container): State<Arc<AppContainer>>,
    Path(slide_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<TakeGroupResponse>, ApiError> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default()
        .trim();
    let extension = match content_type {
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/webm" => "webm",
        _ => return Err(CoreError::Invalid("音訊格式只支援 WebM 或 WAV".to_string()).into()),
    };
    let duration_sec = headers
        .get("x-duration-sec")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    let app = container.clone();
    let group =
        task::spawn_blocking(move || app.save_take(&slide_id, duration_sec, extension, &body))
            .await
            .map_err(|error| CoreError::Conflict(format!("錄音儲存工作失敗：{error}")))??;
    Ok(Json(group.into()))
}

#[derive(serde::Deserialize)]
struct StartRecordingInput {
    slide_id: String,
}

async fn start_recording(
    State(container): State<Arc<AppContainer>>,
    Json(payload): Json<StartRecordingInput>,
) -> Result<Json<crate::recording_service::RecordingStatus>, ApiError> {
    let app = container.clone();
    let status = task::spawn_blocking(move || app.start_recording(&payload.slide_id))
        .await
        .map_err(|error| CoreError::Conflict(format!("啟動錄音失敗：{error}")))??;
    Ok(Json(status))
}

async fn stop_recording(
    State(container): State<Arc<AppContainer>>,
) -> Result<Json<TakeGroupResponse>, ApiError> {
    let app = container.clone();
    let group = task::spawn_blocking(move || app.stop_recording())
        .await
        .map_err(|error| CoreError::Conflict(format!("停止錄音失敗：{error}")))??;
    Ok(Json(group.into()))
}

async fn recording_status(
    State(container): State<Arc<AppContainer>>,
) -> Result<Json<crate::recording_service::RecordingStatus>, ApiError> {
    Ok(Json(container.recording_status()?))
}

async fn set_golden_take(
    State(container): State<Arc<AppContainer>>,
    Path(slide_id): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<TakeGroupResponse>, ApiError> {
    let take_id = payload
        .get("take_id")
        .and_then(Value::as_str)
        .ok_or_else(|| CoreError::Invalid("請提供 take_id".to_string()))?;
    Ok(Json(container.set_golden_take(&slide_id, take_id)?.into()))
}

async fn update_take_note(
    State(container): State<Arc<AppContainer>>,
    Path((slide_id, take_id)): Path<(String, String)>,
    Json(payload): Json<NoteInput>,
) -> Result<Json<TakeGroupResponse>, ApiError> {
    Ok(Json(
        container
            .update_note(&slide_id, &take_id, &payload.note)?
            .into(),
    ))
}

async fn delete_slide_take(
    State(container): State<Arc<AppContainer>>,
    Path((slide_id, take_id)): Path<(String, String)>,
) -> Result<Json<TakeGroupResponse>, ApiError> {
    Ok(Json(container.delete_take(&slide_id, &take_id)?.into()))
}

async fn get_manifest(
    State(container): State<Arc<AppContainer>>,
) -> Result<Json<crate::types::PresentationManifest>, ApiError> {
    Ok(Json(container.manifest()?))
}

#[derive(serde::Deserialize)]
struct DeckIngestInput {
    file_path: PathBuf,
    #[serde(default)]
    parser_profile: Option<DeckParserProfile>,
}

async fn get_deck(
    State(container): State<Arc<AppContainer>>,
) -> Result<Json<Option<crate::app_container::DeckInfo>>, ApiError> {
    Ok(Json(container.deck_info()?))
}

async fn ingest_deck(
    State(container): State<Arc<AppContainer>>,
    Json(payload): Json<DeckIngestInput>,
) -> Result<Json<crate::app_container::DeckInfo>, ApiError> {
    let app = container.clone();
    let deck =
        task::spawn_blocking(move || app.ingest_deck(&payload.file_path, payload.parser_profile))
            .await
            .map_err(|error| CoreError::Conflict(format!("簡報解析工作失敗：{error}")))??;
    Ok(Json(deck))
}

async fn test_parser_profile(
    State(container): State<Arc<AppContainer>>,
    Json(profile): Json<Option<DeckParserProfile>>,
) -> Result<Json<crate::types::ParserTestResult>, ApiError> {
    let app = container.clone();
    let result = task::spawn_blocking(move || app.test_parser_profile(profile.as_ref()))
        .await
        .map_err(|error| CoreError::Conflict(format!("解析預覽工作失敗：{error}")))?;
    let result = result.unwrap_or_else(|error| crate::types::ParserTestResult {
        total_slides: 0,
        slides: Vec::new(),
        error: Some(error.to_string()),
    });
    Ok(Json(result))
}

async fn apply_parser_profile(
    State(container): State<Arc<AppContainer>>,
    Json(profile): Json<DeckParserProfile>,
) -> Result<Json<crate::app_container::DeckInfo>, ApiError> {
    let app = container.clone();
    let deck = task::spawn_blocking(move || app.apply_parser_profile(profile))
        .await
        .map_err(|error| CoreError::Conflict(format!("套用解析規則失敗：{error}")))??;
    Ok(Json(deck))
}

async fn get_slide_notes(
    State(container): State<Arc<AppContainer>>,
    Path(slide_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let deck = container
        .deck_info()?
        .ok_or_else(|| CoreError::NotFound("尚未載入簡報".to_string()))?;
    let slide = deck
        .slides
        .iter()
        .find(|slide| slide.slide_id == slide_id)
        .ok_or_else(|| CoreError::NotFound("找不到此投影片".to_string()))?;
    Ok(Json(json!({"slide_id": slide_id, "notes": slide.notes})))
}

async fn export_master_audio(
    State(container): State<Arc<AppContainer>>,
) -> Result<Json<crate::master_export::MasterAudioExport>, ApiError> {
    let app = container.clone();
    let export = task::spawn_blocking(move || app.export_master_audio())
        .await
        .map_err(|error| CoreError::Conflict(format!("母帶匯出工作失敗：{error}")))??;
    Ok(Json(export))
}

async fn get_master_audio(
    State(container): State<Arc<AppContainer>>,
    Path(filename): Path<String>,
) -> Result<Response<Body>, ApiError> {
    let path = container.master_audio_path(&filename)?;
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => CoreError::NotFound("找不到匯出的母帶".to_string()),
            _ => CoreError::Io(error),
        })?;
    let file = tokio::fs::File::open(path).await.map_err(CoreError::Io)?;
    let mut response = Response::new(Body::from_stream(ReaderStream::new(file)));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("audio/wav"));
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&metadata.len().to_string())
            .map_err(|error| CoreError::Invalid(error.to_string()))?,
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .map_err(|error| CoreError::Invalid(error.to_string()))?,
    );
    Ok(response)
}

async fn start_video_export(
    State(container): State<Arc<AppContainer>>,
) -> Result<Json<crate::video_export::VideoExportStatus>, ApiError> {
    let app = container.clone();
    let status = task::spawn_blocking(move || app.start_video_export())
        .await
        .map_err(|error| CoreError::Conflict(format!("啟動影片輸出失敗：{error}")))??;
    Ok(Json(status))
}

async fn get_video_export_status(
    State(container): State<Arc<AppContainer>>,
    Path(job_id): Path<String>,
) -> Result<Json<crate::video_export::VideoExportStatus>, ApiError> {
    Ok(Json(container.video_export_status(&job_id)?))
}

async fn finish_video_export(
    State(container): State<Arc<AppContainer>>,
    Path(job_id): Path<String>,
) -> Result<Json<crate::video_export::VideoExportStatus>, ApiError> {
    let app = container.clone();
    let status = task::spawn_blocking(move || app.finish_video_export(&job_id))
        .await
        .map_err(|error| CoreError::Conflict(format!("影片合成工作失敗：{error}")))??;
    Ok(Json(status))
}

#[derive(serde::Deserialize)]
struct VideoExportFailure {
    message: String,
}

async fn fail_video_export(
    State(container): State<Arc<AppContainer>>,
    Path(job_id): Path<String>,
    Json(payload): Json<VideoExportFailure>,
) -> Result<Json<crate::video_export::VideoExportStatus>, ApiError> {
    let message = payload.message.chars().take(1000).collect();
    container.fail_video_export(&job_id, message);
    Ok(Json(container.video_export_status(&job_id)?))
}

async fn upload_video_slide(
    State(container): State<Arc<AppContainer>>,
    Path((job_id, slide_index)): Path<(String, usize)>,
    body: Bytes,
) -> Result<Json<crate::video_export::VideoExportStatus>, ApiError> {
    let app = container.clone();
    let status = task::spawn_blocking(move || app.record_video_slide(&job_id, slide_index, &body))
        .await
        .map_err(|error| CoreError::Conflict(format!("接收投影片影像失敗：{error}")))??;
    Ok(Json(status))
}

async fn get_video(
    State(container): State<Arc<AppContainer>>,
    Path(filename): Path<String>,
) -> Result<AxumResponse, ApiError> {
    let path = container.video_path(&filename)?;
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => CoreError::NotFound("找不到匯出的影片".to_string()),
            _ => CoreError::Io(error),
        })?;
    let file = tokio::fs::File::open(path).await.map_err(CoreError::Io)?;
    let mut response = Response::new(Body::from_stream(ReaderStream::new(file)));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("video/mp4"));
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&metadata.len().to_string())
            .map_err(|error| CoreError::Invalid(error.to_string()))?,
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn get_audio(
    State(container): State<Arc<AppContainer>>,
    Path((slide_id, filename)): Path<(String, String)>,
) -> Result<Response<Body>, ApiError> {
    let path = container.audio_path(&slide_id, &filename)?;
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => CoreError::NotFound("找不到指定的音訊檔案".to_string()),
            _ => CoreError::Io(error),
        })?;
    let file = tokio::fs::File::open(path).await.map_err(CoreError::Io)?;
    let stream = ReaderStream::new(file);
    let mut response = Response::new(Body::from_stream(stream));
    let content_type = if filename.ends_with(".wav") {
        "audio/wav"
    } else {
        "audio/webm"
    };
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&metadata.len().to_string())
            .map_err(|error| CoreError::Invalid(error.to_string()))?,
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn get_bridge_sdk() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        BRIDGE_SDK,
    )
}

async fn get_embedded_ui_sdk() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        EMBEDDED_UI_SDK,
    )
}

async fn get_html2canvas_sdk() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        HTML2CANVAS_SDK,
    )
}

async fn websocket_upgrade(
    State(container): State<Arc<AppContainer>>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> AxumResponse {
    if headers
        .get(header::ORIGIN)
        .is_some_and(|origin| !is_local_origin(origin))
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    upgrade
        .on_upgrade(move |socket| websocket_session(socket, container))
        .into_response()
}

async fn websocket_session(mut socket: WebSocket, container: Arc<AppContainer>) {
    let mut receiver = container.slide_sync_hub.subscribe();
    let mut presentation_ui_attached = false;
    if let Some(active_slide) = container.active_slide() {
        let initial = json!({"type": "slide_changed", "payload": active_slide}).to_string();
        if socket.send(Message::Text(initial.into())).await.is_err() {
            return;
        }
    }
    let embedded_ui_count = container.slide_sync_hub.presentation_ui_connection_count();
    if embedded_ui_count > 0 {
        let initial = json!({
            "type": "embedded_ui_attached",
            "payload": {"count": embedded_ui_count}
        })
        .to_string();
        if socket.send(Message::Text(initial.into())).await.is_err() {
            return;
        }
    }

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(value) = serde_json::from_str::<Value>(text.as_str()) {
                            if let Some(event_type) = value.get("type").and_then(Value::as_str) {
                                let payload = value.get("payload").cloned().unwrap_or(Value::Null);
                                match event_type {
                                    "presentation_ui_attached" if !presentation_ui_attached => {
                                        presentation_ui_attached = true;
                                        container.slide_sync_hub.attach_presentation_ui();
                                    }
                                    "toggle_native_overlay" if payload.get("visible").and_then(Value::as_bool).is_some() => {
                                        container.slide_sync_hub.publish(event_type, payload);
                                    }
                                    "open_native_studio" => {
                                        container.slide_sync_hub.publish(event_type, payload);
                                    }
                                    "video_capture_finished" => {
                                        if let Some(job_id) = payload.get("job_id").and_then(Value::as_str) {
                                            let app = container.clone();
                                            let job_id = job_id.to_string();
                                            task::spawn_blocking(move || {
                                                if let Err(error) = app.finish_video_export(&job_id) {
                                                    app.fail_video_export(&job_id, error.to_string());
                                                }
                                            });
                                        }
                                    }
                                    "video_capture_failed" => {
                                        if let (Some(job_id), Some(message)) = (
                                            payload.get("job_id").and_then(Value::as_str),
                                            payload.get("message").and_then(Value::as_str),
                                        ) {
                                            container.fail_video_export(job_id, message.to_string());
                                        }
                                    }
                                    "start_golden_autoplay" | "stop_golden_autoplay" | "keyboard_shortcut" | "navigate_slide" | "autoplay_finished" | "autoplay_error" => {
                                        container.slide_sync_hub.publish(event_type, payload);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(Message::Binary(_))) | Some(Ok(Message::Pong(_))) => {}
                }
            }
            event = receiver.recv() => {
                match event {
                    Ok(message) => {
                        if socket.send(Message::Text(message.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
    if presentation_ui_attached {
        container.slide_sync_hub.detach_presentation_ui();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tempfile::TempDir;
    use tower::ServiceExt;

    async fn body_json(response: AxumResponse) -> Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn embedded_presentation_ui_sdk_is_served_from_core() {
        let directory = TempDir::new().unwrap();
        let response = router(AppContainer::open(directory.path()).unwrap())
            .oneshot(
                Request::builder()
                    .uri("/sai-slidetake-embedded.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("application/javascript"));
        let script = response.into_body().collect().await.unwrap().to_bytes();
        let script = String::from_utf8(script.to_vec()).unwrap();
        assert!(script.contains("attachShadow"));
        assert!(script.contains("fullscreenchange"));
        assert!(BRIDGE_SDK.contains("presentation_ui_attached"));
    }

    #[tokio::test]
    async fn parser_profile_preview_and_apply_use_the_loaded_html_deck() {
        let directory = TempDir::new().unwrap();
        let source = directory.path().join("profile-test.html");
        std::fs::write(
            &source,
            r#"<article class="slide page present"><h1>Opening</h1><aside class="note">Welcome</aside></article><article class="slide page"><h1>Details</h1></article>"#,
        )
        .unwrap();
        let container = AppContainer::open(directory.path()).unwrap();
        container.ingest_deck(&source, None).unwrap();
        let profile = json!({
            "engine_name": "Custom test",
            "slide_selector": ".page",
            "active_class": "present",
            "title_selector": "h1",
            "notes_selector": ".note",
            "fallback_strategy": "manual"
        });
        let app = router(container);
        let preview = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/deck/test-parser")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(profile.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(preview.status(), StatusCode::OK);
        let preview = body_json(preview).await;
        assert_eq!(preview["total_slides"], 2);
        assert_eq!(preview["slides"][0]["title"], "Opening");
        assert_eq!(preview["slides"][0]["notes"], "Welcome");
        assert_eq!(preview["slides"][0]["active"], true);

        let invalid_profile = json!({
            "engine_name": "Invalid test",
            "slide_selector": "[",
            "fallback_strategy": "manual"
        });
        let invalid = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/deck/test-parser")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(invalid_profile.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(invalid.status(), StatusCode::OK);
        let invalid = body_json(invalid).await;
        assert_eq!(invalid["total_slides"], 0);
        assert!(invalid["error"].as_str().unwrap().contains("selector"));

        let applied = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/deck/apply-parser")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(profile.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(applied.status(), StatusCode::OK);
        let applied = body_json(applied).await;
        assert_eq!(applied["slides"].as_array().unwrap().len(), 2);
        assert_eq!(applied["slides"][1]["title"], "Details");
    }

    #[tokio::test]
    async fn parser_profile_refuses_to_apply_unmappable_hr_split_preview() {
        let directory = TempDir::new().unwrap();
        let source = directory.path().join("hr-preview.html");
        std::fs::write(
            &source,
            r#"<div class="slide"><p>First page</p></div><hr><div class="slide"><p>Second page</p></div>"#,
        )
        .unwrap();
        let container = AppContainer::open(directory.path()).unwrap();
        container.ingest_deck(&source, None).unwrap();
        let profile = json!({
            "engine_name": "HR fallback",
            "slide_selector": ".missing-slide",
            "active_class": "",
            "title_selector": "h1, h2",
            "notes_selector": "",
            "fallback_strategy": "hr_split"
        });
        let response = router(container)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/deck/apply-parser")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(profile.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(body_json(response).await["error"]
            .as_str()
            .unwrap()
            .contains("無法對應"));
    }

    #[tokio::test]
    async fn slide_take_api_preserves_versions_and_manifest_golden_pointer() {
        let directory = TempDir::new().unwrap();
        let container = AppContainer::open(directory.path()).unwrap();
        let app = router(container);

        let active = Request::builder()
            .method("POST")
            .uri("/api/slide/active")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"slide_id":"page-01","slide_index":0,"total_slides":24,"slide_title":"封面"}"#,
            ))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(active).await.unwrap().status(),
            StatusCode::OK
        );

        for duration in ["2.5", "3.0"] {
            let upload = Request::builder()
                .method("POST")
                .uri("/api/slides/page-01/takes")
                .header(header::CONTENT_TYPE, "audio/webm")
                .header("x-duration-sec", duration)
                .body(Body::from(format!("webm-{duration}")))
                .unwrap();
            assert_eq!(
                app.clone().oneshot(upload).await.unwrap().status(),
                StatusCode::OK
            );
        }

        let takes_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/slides/page-01/takes")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let takes = body_json(takes_response).await;
        assert_eq!(takes["golden_take_id"], "take_1");
        assert_eq!(takes["takes"].as_array().unwrap().len(), 2);
        let first_filename = takes["takes"][0]["filename"].as_str().unwrap();
        assert!(first_filename.starts_with("take_1_"));
        let second_filename = takes["takes"][1]["filename"].as_str().unwrap();
        assert!(second_filename.starts_with("take_2_"));

        let golden = Request::builder()
            .method("PUT")
            .uri("/api/slides/page-01/golden")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"take_id":"take_2"}"#))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(golden).await.unwrap().status(),
            StatusCode::OK
        );

        let note = Request::builder()
            .method("PATCH")
            .uri("/api/slides/page-01/takes/take_1")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"note":"第二版比較沉穩"}"#))
            .unwrap();
        let note_response = app.clone().oneshot(note).await.unwrap();
        assert_eq!(note_response.status(), StatusCode::OK);
        let note_group = body_json(note_response).await;
        assert_eq!(note_group["takes"][0]["note"], "第二版比較沉穩");

        let audio = Request::builder()
            .uri(format!("/api/audio/page-01/{second_filename}"))
            .body(Body::empty())
            .unwrap();
        let audio_response = app.clone().oneshot(audio).await.unwrap();
        assert_eq!(audio_response.status(), StatusCode::OK);
        assert_eq!(audio_response.headers()[header::CONTENT_TYPE], "audio/webm");
        let audio_bytes = audio_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        assert_eq!(&audio_bytes[..], b"webm-3.0");

        let delete = Request::builder()
            .method("DELETE")
            .uri("/api/slides/page-01/takes/take_1")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(delete).await.unwrap().status(),
            StatusCode::OK
        );

        let upload_after_delete = Request::builder()
            .method("POST")
            .uri("/api/slides/page-01/takes")
            .header(header::CONTENT_TYPE, "audio/webm")
            .header("x-duration-sec", "4.0")
            .body(Body::from("webm-4.0"))
            .unwrap();
        let after_delete = body_json(app.clone().oneshot(upload_after_delete).await.unwrap()).await;
        assert!(after_delete["takes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|take| take["take_id"] == "take_3"));

        let manifest_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/manifest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let manifest = body_json(manifest_response).await;
        assert_eq!(manifest["slides"]["page-01"]["golden_take_id"], "take_2");
        assert_eq!(
            manifest["slides"]["page-01"]["takes"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let manifest_file = directory.path().join("audio/slidetake-manifest.json");
        assert!(manifest_file.is_file());
        let slide_audio = directory.path().join("audio/page-01");
        assert_eq!(std::fs::read_dir(slide_audio).unwrap().count(), 2);

        let bridge = app
            .oneshot(
                Request::builder()
                    .uri("/sai-slidetake-bridge.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(bridge.status(), StatusCode::OK);
        assert!(bridge.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("application/javascript"));
    }

    #[tokio::test]
    async fn html_deck_ingest_creates_its_own_manifest_namespace() {
        let directory = TempDir::new().unwrap();
        let presentation_root = directory.path().join("initial");
        let source = directory.path().join("demo.html");
        std::fs::write(&source, r#"<title>Demo</title><div class="slides"><section id="intro"><h1>Intro</h1></section><section id="summary" data-title="Summary"></section></div>"#).unwrap();
        let container = AppContainer::open(&presentation_root).unwrap();
        let app = router(container.clone());
        let ingest = Request::builder()
            .method("POST")
            .uri("/api/deck/ingest")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({"file_path": source}).to_string(),
            ))
            .unwrap();
        let response = app.clone().oneshot(ingest).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let deck = body_json(response).await;
        assert_eq!(deck["format"], "html");
        assert_eq!(deck["slides"].as_array().unwrap().len(), 2);
        assert_eq!(deck["slides"][0]["slide_id"], "intro");
        let bridge_copy = deck["bridge_path"].as_str().unwrap();
        assert_ne!(bridge_copy, deck["source_path"].as_str().unwrap());
        assert!(std::fs::read_to_string(bridge_copy)
            .unwrap()
            .contains("sai-slidetake-bridge.js"));
        assert!(
            !std::fs::read_to_string(deck["source_path"].as_str().unwrap())
                .unwrap()
                .contains("sai-slidetake-bridge.js")
        );

        let manifest = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/manifest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let manifest = body_json(manifest).await;
        assert_eq!(manifest["presentation_id"], deck["deck_id"]);
        assert_eq!(manifest["slides"]["summary"]["slide_title"], "Summary");
        let separate_manifest = presentation_root
            .join("slidetake_decks/decks")
            .join(deck["deck_id"].as_str().unwrap())
            .join("audio/slidetake-manifest.json");
        assert!(separate_manifest.is_file());
        let initial_manifest: Value = serde_json::from_slice(
            &std::fs::read(presentation_root.join("audio/slidetake-manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(initial_manifest["presentation_id"], "initial");
        assert_ne!(initial_manifest["presentation_id"], deck["deck_id"]);
    }

    #[tokio::test]
    async fn websocket_broadcasts_active_slide_changes_and_cors_stays_local() {
        use futures_util::{SinkExt, StreamExt};
        use std::time::Instant;
        use tokio_tungstenite::tungstenite::Message as ClientMessage;

        let directory = TempDir::new().unwrap();
        let container = AppContainer::open(directory.path()).unwrap();
        let app = router(container.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = task::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/ws"))
            .await
            .unwrap();

        socket
            .send(ClientMessage::Text(
                r#"{"type":"start_golden_autoplay","payload":{"start_slide_index":0}}"#.into(),
            ))
            .await
            .unwrap();
        let _ = socket.next().await.unwrap().unwrap();
        socket
            .send(ClientMessage::Text(
                r#"{"type":"presentation_ui_attached","payload":{}}"#.into(),
            ))
            .await
            .unwrap();
        let attached: Value =
            serde_json::from_str(&socket.next().await.unwrap().unwrap().into_text().unwrap())
                .unwrap();
        assert_eq!(attached["type"], "embedded_ui_attached");
        assert_eq!(attached["payload"]["count"], 1);
        assert_eq!(
            container.slide_sync_hub.presentation_ui_connection_count(),
            1
        );

        let active = Request::builder()
            .method("POST")
            .uri("/api/slide/active")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"slide_id":"page-02","slide_index":1,"total_slides":24,"slide_title":"第二頁"}"#,
            ))
            .unwrap();
        let sent_at = Instant::now();
        let _ = router(container.clone()).oneshot(active).await.unwrap();
        let message = socket.next().await.unwrap().unwrap().into_text().unwrap();
        let latency = sent_at.elapsed();
        println!(
            "slide_changed HTTP-to-WebSocket latency: {:.3} ms",
            latency.as_secs_f64() * 1000.0
        );
        assert!(
            latency.as_millis() < 16,
            "local slide sync took {latency:?}"
        );
        let event: Value = serde_json::from_str(&message).unwrap();
        assert_eq!(event["type"], "slide_changed");
        assert_eq!(event["payload"]["slide_id"], "page-02");

        let accepted = Request::builder()
            .uri("/api/manifest")
            .header(header::ORIGIN, "http://localhost:1425")
            .body(Body::empty())
            .unwrap();
        let accepted = router(container.clone()).oneshot(accepted).await.unwrap();
        assert_eq!(
            accepted.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
            "http://localhost:1425"
        );
        let rejected = Request::builder()
            .uri("/api/manifest")
            .header(header::ORIGIN, "https://outside.example")
            .body(Body::empty())
            .unwrap();
        let rejected = router(container.clone()).oneshot(rejected).await.unwrap();
        assert!(!rejected
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));

        socket.close(None).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert_eq!(
            container.slide_sync_hub.presentation_ui_connection_count(),
            0
        );
        server.abort();
    }

    #[test]
    fn path_validation_rejects_traversal_segments() {
        assert!(crate::audio_storage::validate_slide_id("page-01").is_ok());
        assert!(crate::audio_storage::validate_slide_id("../escape").is_err());
        assert!(
            crate::audio_storage::validate_audio_filename("take_1_20261109_203000.webm").is_ok()
        );
        assert!(crate::audio_storage::validate_audio_filename("../../outside.webm").is_err());
        assert!(
            crate::audio_storage::validate_audio_filename("take_1_20261111_203000.wav").is_ok()
        );
    }

    #[tokio::test]
    async fn recording_status_is_readable_without_opening_the_microphone() {
        let directory = TempDir::new().unwrap();
        let app = router(AppContainer::open(directory.path()).unwrap());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/recording/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let status = body_json(response).await;
        assert_eq!(status["active"], false);
        assert_eq!(status["level"], 0.0);
    }

    #[test]
    fn browser_origins_are_limited_to_local_decks_and_tauri() {
        assert!(is_local_origin(&HeaderValue::from_static("null")));
        assert!(is_local_origin(&HeaderValue::from_static(
            "tauri://localhost"
        )));
        assert!(is_local_origin(&HeaderValue::from_static(
            "http://localhost:1425"
        )));
        assert!(is_local_origin(&HeaderValue::from_static(
            "http://127.0.0.1:3188"
        )));
        assert!(!is_local_origin(&HeaderValue::from_static(
            "https://example.com"
        )));
        assert!(!is_local_origin(&HeaderValue::from_static(
            "https://localhost.example.com"
        )));
    }

    #[tokio::test]
    async fn re_ingesting_presentation_from_different_path_auto_relinks_and_preserves_takes() {
        let directory = TempDir::new().unwrap();
        let container = AppContainer::open(directory.path()).unwrap();
        let app = router(container.clone());

        // 模擬電腦 A：於 dir_a 建立簡報並匯入
        let dir_a = TempDir::new().unwrap();
        let html_a = dir_a.path().join("report.html");
        std::fs::write(
            &html_a,
            r#"<!doctype html><html><body><section class="slide" id="slide-1"><h1>Title 1</h1></section><section class="slide" id="slide-2"><h1>Title 2</h1></section></body></html>"#,
        )
        .unwrap();

        let resp_a = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/deck/ingest")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"file_path": html_a.to_string_lossy()}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp_a.status(), StatusCode::OK);
        let deck_a = body_json(resp_a).await;
        let deck_id_a = deck_a["deck_id"].as_str().unwrap().to_string();

        // 於 slide-1 新增一個錄音 Take
        let save_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/slides/slide-1/takes")
                    .header(header::CONTENT_TYPE, "audio/webm")
                    .header("x-duration-sec", "3.5")
                    .header("x-take-note", "Take on Machine A")
                    .body(Body::from("webm-content-a"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(save_resp.status(), StatusCode::OK);

        // 刪除 dir_a，模擬搬移到另一台電腦（原路徑已失效）
        drop(dir_a);

        // 模擬電腦 B：於全新的 dir_b 存放同一份 report.html 簡報
        let dir_b = TempDir::new().unwrap();
        let html_b = dir_b.path().join("report.html");
        std::fs::write(
            &html_b,
            r#"<!doctype html><html><body><section class="slide" id="slide-1"><h1>Title 1 Updated</h1></section><section class="slide" id="slide-2"><h1>Title 2</h1></section></body></html>"#,
        )
        .unwrap();

        // 重新拖入 / 匯入 dir_b 中的 report.html
        let resp_b = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/deck/ingest")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"file_path": html_b.to_string_lossy()}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp_b.status(), StatusCode::OK);
        let deck_b = body_json(resp_b).await;
        let deck_id_b = deck_b["deck_id"].as_str().unwrap();

        // 驗證 1: 自動識別並重連至同一個既有 deck_id
        assert_eq!(deck_id_b, deck_id_a);
        // 驗證 2: 來源路徑已自動更新為電腦 B 的新路徑
        assert_eq!(
            deck_b["source_path"].as_str().unwrap(),
            html_b.canonicalize().unwrap().to_string_lossy()
        );

        // 驗證 3: slide-1 原先在電腦 A 錄製的 Take 完整保留無失！
        let takes_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/slides/slide-1/takes")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(takes_resp.status(), StatusCode::OK);
        let takes_body = body_json(takes_resp).await;
        assert_eq!(takes_body["takes"].as_array().unwrap().len(), 1);
    }
}

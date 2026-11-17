use serde_json::Value;
use std::{
    net::SocketAddr,
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{AppHandle, Manager, WebviewWindow};

const APP_CONFIG: &str = include_str!("../../../SAISlideTake.json");

#[derive(Default)]
struct OverlayRestoreGeometry(
    Mutex<Option<(tauri::PhysicalPosition<i32>, tauri::PhysicalSize<u32>)>>,
);

struct OverlayTopmostState(Arc<AtomicBool>);

impl Default for OverlayTopmostState {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }
}

#[tauri::command]
fn resize_overlay(
    window: WebviewWindow,
    state: tauri::State<'_, OverlayRestoreGeometry>,
    topmost_state: tauri::State<'_, OverlayTopmostState>,
    mode: String,
) -> Result<(), String> {
    let keep_topmost_on_blur = is_overlay_mode(&mode);
    if !keep_topmost_on_blur && mode != "studio" {
        return Err("未知的浮動視窗模式".to_string());
    }
    topmost_state
        .0
        .store(keep_topmost_on_blur, Ordering::Release);

    match mode.as_str() {
        "studio" => {
            window
                .set_always_on_top(false)
                .map_err(|error| error.to_string())?;
            let mut restore = state
                .inner()
                .0
                .lock()
                .map_err(|_| "浮動視窗位置狀態鎖已失效".to_string())?;
            if restore.is_none() {
                *restore = Some((
                    window.outer_position().map_err(|error| error.to_string())?,
                    window.outer_size().map_err(|error| error.to_string())?,
                ));
            }
            drop(restore);
            window
                .set_fullscreen(false)
                .map_err(|error| error.to_string())?;
            window.unmaximize().map_err(|error| error.to_string())?;
            window
                .set_decorations(false)
                .map_err(|error| error.to_string())?;
            window
                .set_min_size(Some(tauri::LogicalSize::new(720.0, 520.0)))
                .map_err(|error| error.to_string())?;
            window
                .set_max_size::<tauri::Size>(None)
                .map_err(|error| error.to_string())?;
            window
                .set_resizable(true)
                .map_err(|error| error.to_string())?;
            window
                .set_size(tauri::Size::Logical(tauri::LogicalSize::new(1080.0, 720.0)))
                .map_err(|error| error.to_string())?;
            window.center().map_err(|error| error.to_string())?;
        }
        "drawer" => {
            window.unmaximize().map_err(|error| error.to_string())?;
            window
                .set_fullscreen(false)
                .map_err(|error| error.to_string())?;
            window
                .set_decorations(false)
                .map_err(|error| error.to_string())?;
            window
                .set_resizable(false)
                .map_err(|error| error.to_string())?;
            window
                .set_min_size(Some(tauri::LogicalSize::new(360.0, 56.0)))
                .map_err(|error| error.to_string())?;
            window
                .set_max_size(Some(tauri::LogicalSize::new(420.0, 480.0)))
                .map_err(|error| error.to_string())?;
            window
                .set_size(tauri::Size::Logical(tauri::LogicalSize::new(420.0, 480.0)))
                .map_err(|error| error.to_string())?;
            restore_overlay_position(&window, &state)?;
        }
        "capsule" => {
            window.unmaximize().map_err(|error| error.to_string())?;
            window
                .set_fullscreen(false)
                .map_err(|error| error.to_string())?;
            window
                .set_decorations(false)
                .map_err(|error| error.to_string())?;
            window
                .set_resizable(false)
                .map_err(|error| error.to_string())?;
            window
                .set_min_size(Some(tauri::LogicalSize::new(360.0, 56.0)))
                .map_err(|error| error.to_string())?;
            window
                .set_max_size(Some(tauri::LogicalSize::new(420.0, 480.0)))
                .map_err(|error| error.to_string())?;
            window
                .set_size(tauri::Size::Logical(tauri::LogicalSize::new(360.0, 56.0)))
                .map_err(|error| error.to_string())?;
            restore_overlay_position(&window, &state)?;
        }
        _ => return Err("未知的浮動視窗模式".to_string()),
    }
    if keep_topmost_on_blur {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        window
            .set_shadow(false)
            .map_err(|error| error.to_string())?;
        window
            .set_always_on_top(true)
            .map_err(|error| error.to_string())?;
        #[cfg(target_os = "macos")]
        configure_macos_fullscreen_overlay(&window).map_err(|error| error.to_string())?;
    } else {
        #[cfg(target_os = "macos")]
        set_macos_studio_workspace_behavior(&window).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn is_overlay_mode(mode: &str) -> bool {
    matches!(mode, "capsule" | "drawer")
}

fn restore_overlay_position(
    window: &WebviewWindow,
    state: &tauri::State<'_, OverlayRestoreGeometry>,
) -> Result<(), String> {
    let geometry = state
        .inner()
        .0
        .lock()
        .map_err(|_| "浮動視窗位置狀態鎖已失效".to_string())?
        .take();
    if let Some((position, _)) = geometry {
        window
            .set_position(tauri::Position::Physical(position))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn open_html_presentation(path: String) -> Result<(), String> {
    let path = PathBuf::from(path)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "html" | "htm") {
        return Err("橋接簡報必須是 HTML 檔案".to_string());
    }
    let source = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    if !source.contains("sai-slidetake-bridge.js") {
        return Err("找不到簡報橋接腳本".to_string());
    }

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(&path);
        command
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", ""]).arg(&path);
        command
    };
    #[cfg(target_os = "linux")]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(&path);
        command
    };
    command.spawn().map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn exit_application(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn minimize_studio_window(window: WebviewWindow) -> Result<(), String> {
    window.minimize().map_err(|error| error.to_string())
}

#[tauri::command]
fn toggle_studio_window_maximize(window: WebviewWindow) -> Result<bool, String> {
    let maximized = window.is_maximized().map_err(|error| error.to_string())?;
    if maximized {
        window.unmaximize().map_err(|error| error.to_string())?;
        Ok(false)
    } else {
        window
            .set_resizable(true)
            .map_err(|error| error.to_string())?;
        window
            .set_min_size(Some(tauri::LogicalSize::new(720.0, 520.0)))
            .map_err(|error| error.to_string())?;
        window
            .set_max_size::<tauri::Size>(None)
            .map_err(|error| error.to_string())?;
        window.maximize().map_err(|error| error.to_string())?;
        Ok(true)
    }
}

#[tauri::command]
fn start_studio_window_dragging(window: WebviewWindow) -> Result<(), String> {
    window.start_dragging().map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn configure_macos_fullscreen_overlay(
    window: &WebviewWindow,
) -> Result<(), Box<dyn std::error::Error>> {
    use objc2_app_kit::{NSFloatingWindowLevel, NSWindow, NSWindowCollectionBehavior};

    window.set_visible_on_all_workspaces(true)?;
    let native_window = window.ns_window()?;
    let native_window = unsafe { (native_window as *mut NSWindow).as_ref() }
        .ok_or_else(|| std::io::Error::other("Tauri 視窗尚未建立 NSWindow"))?;
    native_window.setCollectionBehavior(
        native_window.collectionBehavior()
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::CanJoinAllSpaces,
    );
    native_window.setLevel(NSFloatingWindowLevel);
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_macos_studio_workspace_behavior(
    window: &WebviewWindow,
) -> Result<(), Box<dyn std::error::Error>> {
    use objc2_app_kit::{NSNormalWindowLevel, NSWindow, NSWindowCollectionBehavior};

    window.set_visible_on_all_workspaces(false)?;
    let native_window = window.ns_window()?;
    let native_window = unsafe { (native_window as *mut NSWindow).as_ref() }
        .ok_or_else(|| std::io::Error::other("Tauri 視窗尚未建立 NSWindow"))?;
    native_window.setCollectionBehavior(
        native_window.collectionBehavior() & !NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    native_window.setLevel(NSNormalWindowLevel);
    Ok(())
}

#[cfg(target_os = "windows")]
fn install_windows_topmost_reassertion(window: &WebviewWindow, topmost_state: Arc<AtomicBool>) {
    let window = window.clone();
    let event_window = window.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Focused(false))
            && topmost_state.load(Ordering::Acquire)
        {
            let _ = event_window.set_always_on_top(false);
            let _ = event_window.set_always_on_top(true);
        }
    });
}

fn is_writable_deck_dir(path: &std::path::Path) -> bool {
    // 排除系統根目錄與空路徑
    if path == std::path::Path::new("/") || path.as_os_str().is_empty() || path.parent().is_none() {
        return false;
    }
    // 排除 Windows 磁碟根目錄如 C:\ 或 D:\
    #[cfg(target_os = "windows")]
    {
        let path_str = path.to_string_lossy();
        if path_str.len() <= 3 && path_str.ends_with(":\\") {
            return false;
        }
    }
    // 測試寫入權限（嘗試建立臨時探測目錄）
    let test_probe = path.join(".saist_write_test");
    if std::fs::create_dir_all(&test_probe).is_ok() {
        let _ = std::fs::remove_dir(&test_probe);
        return true;
    }
    false
}

fn fallback_presentation_directory() -> PathBuf {
    let base_dir = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);

    if let Some(home) = base_dir {
        let docs = home.join("Documents").join("SAISlideTake");
        if std::fs::create_dir_all(&docs).is_ok() {
            return docs;
        }
        let home_deck = home.join("SAISlideTake");
        if std::fs::create_dir_all(&home_deck).is_ok() {
            return home_deck;
        }
    }

    let temp_deck = std::env::temp_dir().join("SAISlideTake");
    let _ = std::fs::create_dir_all(&temp_deck);
    temp_deck
}

/// 將設定檔中的絕對或相對目錄解析為可寫入的絕對路徑。
fn resolve_configured_presentation_dir(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let raw_path = PathBuf::from(trimmed);
    let resolved = if raw_path.is_absolute() {
        raw_path
    } else {
        let cwd = std::env::current_dir().ok()?;
        if !is_writable_deck_dir(&cwd) {
            return None;
        }
        let mut base = cwd;
        for component in raw_path.components() {
            match component {
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    base.pop();
                }
                std::path::Component::Normal(part) => base.push(part),
                std::path::Component::RootDir | std::path::Component::Prefix(_) => {}
            }
        }
        base
    };

    if is_writable_deck_dir(&resolved) {
        Some(resolved)
    } else {
        None
    }
}

fn presentation_directory() -> PathBuf {
    if let Some(path) = std::env::var_os("SAIST_PRESENTATION_DIR") {
        let p = PathBuf::from(path);
        if !p.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(&p);
            return p;
        }
    }

    // 檢查鄰近目錄是否包含 SAISlideTake_Data（自包含可攜式架構支援）
    if let Ok(exe_path) = std::env::current_exe() {
        let mut cur = exe_path.as_path();
        for _ in 0..6 {
            if let Some(parent) = cur.parent() {
                let candidate = parent.join("SAISlideTake_Data");
                if candidate.is_dir() {
                    return candidate;
                }
                cur = parent;
            } else {
                break;
            }
        }
    }

    let config: Value = serde_json::from_str(APP_CONFIG).unwrap_or_default();
    if let Some(resolved) = config
        .get("default_presentation_dir")
        .and_then(Value::as_str)
        .and_then(resolve_configured_presentation_dir)
    {
        return resolved;
    }

    // 檢查當前工作目錄是否合適（非根目錄且具備寫入能力）
    if let Ok(cwd) = std::env::current_dir() {
        if is_writable_deck_dir(&cwd) {
            return cwd;
        }
    }

    // 回退至使用者 Documents/SAISlideTake 安全目錄
    fallback_presentation_directory()
}

fn start_core_service(_app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let deck_dir = presentation_directory();
    let container = saist_core::AppContainer::open(&deck_dir)?;
    let address = SocketAddr::from(([127, 0, 0, 1], 3188));
    tauri::async_runtime::spawn(async move {
        if let Err(error) = saist_core::serve(address, container).await {
            eprintln!("[SAISlideTake] Core service stopped: {error}");
        }
    });
    println!(
        "[SAISlideTake] Core service requested for {}",
        deck_dir.display()
    );
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .manage(OverlayRestoreGeometry::default())
        .manage(OverlayTopmostState::default())
        .invoke_handler(tauri::generate_handler![
            resize_overlay,
            open_html_presentation,
            minimize_studio_window,
            toggle_studio_window_maximize,
            start_studio_window_dragging,
            exit_application
        ])
        .setup(|app| {
            start_core_service(app.handle())?;
            let window = app.get_webview_window("main").ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "找不到 main 浮動視窗")
            })?;
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            let _ = window.set_shadow(true);
            let _ = window.set_always_on_top(true);
            #[cfg(target_os = "macos")]
            if let Err(error) = configure_macos_fullscreen_overlay(&window) {
                eprintln!("[WARN] configure_macos_fullscreen_overlay: {error}");
            }
            #[cfg(target_os = "windows")]
            install_windows_topmost_reassertion(
                &window,
                app.state::<OverlayTopmostState>().0.clone(),
            );
            let _ = window.show();
            let _ = window.set_focus();
            #[cfg(debug_assertions)]
            window.open_devtools();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("SAISlideTake 桌面浮動視窗啟動失敗");
}

#[cfg(test)]
mod tests {
    use super::{
        is_overlay_mode, is_writable_deck_dir, presentation_directory,
        resolve_configured_presentation_dir,
    };
    use std::path::Path;

    #[test]
    fn capsule_and_drawer_remain_overlay_modes() {
        assert!(is_overlay_mode("capsule"));
        assert!(is_overlay_mode("drawer"));
    }

    #[test]
    fn studio_is_a_regular_window_mode() {
        assert!(!is_overlay_mode("studio"));
    }

    #[test]
    fn root_dir_is_not_writable_deck_dir() {
        assert!(!is_writable_deck_dir(Path::new("/")));
    }

    #[test]
    fn presentation_directory_never_returns_root() {
        let dir = presentation_directory();
        assert_ne!(dir, Path::new("/"));
        assert!(dir.is_absolute());
    }

    #[test]
    fn relative_configured_dir_resolves_to_absolute_path() {
        let dir = resolve_configured_presentation_dir("./SAISlideTake_Data")
            .expect("relative configured path should resolve under writable cwd");
        assert!(dir.is_absolute());
        assert!(dir.ends_with("SAISlideTake_Data"));
        let _ = std::fs::remove_dir(&dir);
    }
}

//! # InvoiceVault
//!
//! 智能发票管理桌面应用，支持 OCR 识别、LLM 提取、语义搜索、
//! Agent 对话和 Excel 模板导出。
//!
//! ## 架构概览
//!
//! - `AppState` — 应用全局状态，管理数据库、配置和各子系统
//! - `commands` — Tauri 前端命令处理层（薄壳，解包参数后调用 AppState）
//! - [`extractor`] — 发票数据提取与 CRUD
//! - `llm` — LLM 客户端（OpenAI 兼容协议）
//! - `agent` — Agent 会话、工具调用和任务管理
//! - [`template_engine`] — Excel 模板导出引擎
//! - [`storage`] — SQLite 数据库迁移

mod agent;
mod app_core;
mod chroma;
mod commands;
mod dedupe;
mod diag;
mod document;
mod email_manager;
mod embedding;
mod event;
pub mod exporter;
pub mod extractor;
mod importer;
mod llm;
pub mod mcp;
mod process_utils;
mod raw_store;
pub mod scnet_ocr;
pub mod storage;
pub mod template_engine;
mod watcher;

use commands::*;
use app_core::AppState;
use app_core::constants::{
    DIR_LOGS, DIR_MODELS, EMBEDDING_MODEL_DIR, EMBEDDING_ONNX_PATH,
    SINGLE_INSTANCE_LOCK_FILE, SINGLE_INSTANCE_TCP_TIMEOUT_MS,
};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WindowEvent,
};
#[cfg(target_os = "windows")]
use tauri::WebviewWindow;
#[cfg(not(target_os = "windows"))]
use tauri::{DragDropEvent, Emitter};
use tracing::{error, info, warn};
use tracing_subscriber::prelude::*;

/// Global reload handle for runtime log level changes.
static LOG_RELOAD_HANDLE: std::sync::OnceLock<
    tracing_subscriber::reload::Handle<
        tracing_subscriber::EnvFilter,
        tracing_subscriber::Registry,
    >,
> = std::sync::OnceLock::new();

/// Apply a new log level at runtime by reloading the tracing filter.
pub fn apply_log_level(level: &str) -> Result<(), String> {
    let filter = tracing_subscriber::EnvFilter::try_new(format!("invoicevault={}", level))
        .map_err(|e| format!("invalid log level '{}': {}", level, e))?;
    LOG_RELOAD_HANDLE
        .get()
        .ok_or("log reload handle not initialized".to_owned())?
        .reload(filter)
        .map_err(|e| format!("failed to reload log filter: {}", e))
}

#[derive(Debug, Serialize, Deserialize)]
struct WindowSizeState {
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, Serialize)]
struct NativeDragStateEvent {
    dragging: bool,
}


const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "main-tray";
const TRAY_WORKBENCH_ID: &str = "tray-workbench";
const TRAY_VERSION_ID: &str = "tray-version";
const TRAY_QUIT_ID: &str = "tray-quit";
const SINGLE_INSTANCE_BIND_ADDR: &str = "127.0.0.1:0";
const SINGLE_INSTANCE_PING_MESSAGE: &[u8] = b"invoicevault:ping\n";
const SINGLE_INSTANCE_SHOW_MESSAGE: &[u8] = b"invoicevault:show\n";
const SINGLE_INSTANCE_OK_MESSAGE: &[u8] = b"invoicevault:ok\n";
const DEFAULT_WINDOW_WIDTH: f64 = 1260.0;
const DEFAULT_WINDOW_HEIGHT: f64 = 860.0;
const MIN_WINDOW_WIDTH: f64 = 1060.0;
const MIN_WINDOW_HEIGHT: f64 = 760.0;

#[cfg(target_os = "windows")]
fn windows_webview_zoom_for_scale(scale_factor: f64) -> f64 {
    if !scale_factor.is_finite() || scale_factor <= 1.25 {
        1.0
    } else if scale_factor <= 1.5 {
        0.94
    } else if scale_factor <= 1.75 {
        0.9
    } else {
        0.86
    }
}

#[cfg(target_os = "windows")]
fn configure_bundled_windows_dependencies(resource_dir: &Path) {
    let deps_dir = [
        resource_dir.join("win-x86_64"),
        resource_dir.join("resources").join("win-x86_64"),
    ]
    .into_iter()
    .find(|path| path.exists());
    let Some(deps_dir) = deps_dir else {
        return;
    };

    std::env::set_var("INVOICEVAULT_WIN_DEPS_DIR", &deps_dir);

    let onnx_runtime_path = deps_dir.join("onnxruntime.dll");
    if onnx_runtime_path.exists() {
        std::env::set_var("ORT_DYLIB_PATH", &onnx_runtime_path);
    }

    let path_separator = ";";
    let mut path_entries = vec![
        deps_dir.clone(),
        deps_dir.join("poppler").join("bin"),
        deps_dir.join("poppler").join("Library").join("bin"),
        deps_dir.join("poppler"),
    ];
    path_entries.retain(|path| path.exists());

    if path_entries.is_empty() {
        return;
    }

    let current_path = std::env::var_os("PATH").unwrap_or_default();
    let mut merged_path = path_entries
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(path_separator);

    if !current_path.is_empty() {
        merged_path.push_str(path_separator);
        merged_path.push_str(&current_path.to_string_lossy());
    }

    std::env::set_var("PATH", merged_path);
}

#[cfg(target_os = "windows")]
fn apply_windows_dpi_zoom(window: &WebviewWindow, scale_factor: f64) {
    let zoom = windows_webview_zoom_for_scale(scale_factor);
    if let Err(err) = window.set_zoom(zoom) {
        warn!("Failed to apply Windows WebView zoom for DPI scale {scale_factor}: {err}");
    }
}

struct SingleInstanceGuard {
    listener: Option<TcpListener>,
    lock_path: std::path::PathBuf,
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.lock_path);
    }
}

/// 启动 InvoiceVault 应用入口，初始化所有插件、状态和窗口。
pub fn run() {
    let Some(mut single_instance_guard) = claim_single_instance() else {
        return;
    };
    let single_instance_listener = single_instance_guard
        .listener
        .take()
        .expect("single instance listener");

    // Workaround: WebKitGTK's compositor thread can deadlock on some Linux systems
    // (especially aarch64 with Mesa/Rockchip GPU drivers), causing the entire
    // webview to freeze. Disabling compositing mode forces all rendering onto the
    // main thread, avoiding the compositor thread bug.
    // See: https://bugs.webkit.org/show_bug.cgi?id=263930
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    }

    // Additional AppImage-specific workarounds
    if std::env::var("APPIMAGE").is_ok() {
        std::env::set_var("GDK_BACKEND", "x11");
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(move |app| {
            // Set up logging directory and file-based tracing subscriber
            let app_data_dir = app.path().app_data_dir().expect("app data dir");
            let log_dir = app_data_dir.join(DIR_LOGS);
            std::fs::create_dir_all(&log_dir).expect("create log dir");

            let file_appender = tracing_appender::rolling::daily(&log_dir, "invoicevault");
            let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

            // Read persisted log level, fallback to info
            let persisted_level = crate::app_core::config::load_config_raw::<serde_json::Value>(
                &app_data_dir,
                "log_config.json",
            )
            .and_then(|v| v.get("level").and_then(|v| v.as_str().map(|s| s.to_owned())))
            .unwrap_or_else(|| "info".to_owned());

            let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("invoicevault={}", persisted_level).into());

            let (filter_layer, reload_handle) =
                tracing_subscriber::reload::Layer::new(env_filter);

            tracing_subscriber::registry()
                .with(filter_layer)
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_writer(non_blocking)
                        .with_ansi(false),
                )
                .init();

            // Store reload handle globally for runtime log level changes
            LOG_RELOAD_HANDLE
                .set(reload_handle)
                .unwrap_or_else(|_| tracing::warn!("log reload handle already set"));

            // Keep the guard alive — when dropped it flushes remaining logs
            // Leak it so it lives for the lifetime of the app
            std::mem::forget(_guard);

            // On macOS/Windows, ONNX Runtime is loaded dynamically at runtime.
            // Set ORT_DYLIB_PATH to the bundled resource directory if available.
            #[cfg(not(target_os = "linux"))]
            {
                if let Some(resource_dir) = app.path().resource_dir().ok() {
                    #[cfg(target_os = "windows")]
                    configure_bundled_windows_dependencies(&resource_dir);

                    let lib_name = if cfg!(target_os = "macos") {
                        "libonnxruntime.dylib"
                    } else {
                        "onnxruntime.dll"
                    };
                    let lib_path = resource_dir.join(lib_name);
                    if lib_path.exists() {
                        std::env::set_var("ORT_DYLIB_PATH", &lib_path);
                    }
                }
            }

            let setup_start = std::time::Instant::now();
            info!("[setup] AppState::initialize: start");
            let state = AppState::initialize(app.handle())?;
            info!(
                "[setup] AppState::initialize: done in {}ms",
                setup_start.elapsed().as_millis()
            );
            app.manage(state);
            setup_tray(app.handle())?;
            setup_single_instance_listener(single_instance_listener, app.handle().clone());

            // NOTE: Embedding engine (ONNX Runtime) loading is intentionally NOT done at startup.
            // The dlopen + ORT global init can conflict with WebKitGTK on Linux, causing the
            // webview to freeze. The engine is loaded lazily when the user triggers
            // test_embedding_connection or regenerate_all_embeddings.

            // Defer watcher directory resumption to a background thread.
            // resume_enabled queries DB and spawns watcher threads per enabled dir.
            {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    let resume_start = std::time::Instant::now();
                    let state = app_handle.state::<AppState>();
                    if let Err(e) = state.resume_watchers() {
                        warn!("[setup] resume_enabled failed: {e}");
                    }
                    info!(
                        "[setup] watcher resume_enabled: done in {}ms",
                        resume_start.elapsed().as_millis()
                    );
                });
            }

            // Preview thumbnail regeneration can be slow (image resize for each invoice).
            // Run it in background so the UI becomes responsive immediately.
            {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    let bg_start = std::time::Instant::now();
                    info!("[bg] regenerate_missing_previews: start");
                    let state = app_handle.state::<AppState>();
                    state.regenerate_missing_previews();
                    info!(
                        "[bg] regenerate_missing_previews: done in {}ms",
                        bg_start.elapsed().as_millis()
                    );
                });
            }

            // Ensure diagnostic sample files are written to app data on first run
            {
                let app_data_dir = app.path().app_data_dir().expect("app data dir");
                diag::ensure_samples(&app_data_dir);
                diag::load_config(&app_data_dir);
            }

            // Background model download for local embedding
            {
                let app_data_dir = app.path().app_data_dir().expect("app data dir");
                // Check if model needs downloading (embedding enabled but engine not loaded)
                let needs_download = {
                    let enabled_json =
                        std::fs::read_to_string(app_data_dir.join("embedding_enabled.json")).ok();
                    let enabled = enabled_json
                        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                        .and_then(|v| v.get("enabled").and_then(|v| v.as_bool()))
                        .unwrap_or(true);
                    let model_dir = app_data_dir.join(DIR_MODELS).join(EMBEDDING_MODEL_DIR);
                    enabled && !model_dir.join(EMBEDDING_ONNX_PATH).exists()
                };
                if needs_download {
                    tauri::async_runtime::spawn(async move {
                        match embedding::ensure_model(&app_data_dir).await {
                            Ok(_model_dir) => {
                                info!("Local embedding model downloaded");
                            }
                            Err(e) => error!("Failed to download embedding model: {e}"),
                        }
                    });
                }
            }

            // Create main window from config with platform-specific drag-drop:
            // - Windows: disable native handler (required for HTML5/DOM drag-drop on WebView2)
            // - Linux/macOS: keep native handler enabled (WebKitGTK displays images without it)
            let window_config = &app.config().app.windows[0];
            #[cfg(target_os = "windows")]
            {
                tauri::WebviewWindowBuilder::from_config(app.handle(), window_config)
                    .expect("failed to create window from config")
                    .disable_drag_drop_handler()
                    .build()
                    .expect("failed to create main window");
            }
            #[cfg(not(target_os = "windows"))]
            {
                tauri::WebviewWindowBuilder::from_config(app.handle(), window_config)
                    .expect("failed to create window from config")
                    .build()
                    .expect("failed to create main window");
            }

            // Restore and persist window size
            let window = app
                .get_webview_window(MAIN_WINDOW_LABEL)
                .expect("main window");
            let state_path = app
                .path()
                .app_data_dir()
                .expect("app data dir")
                .join("window_state.json");

            if let Ok(json) = std::fs::read_to_string(&state_path) {
                if let Ok(saved) = serde_json::from_str::<WindowSizeState>(&json) {
                    use tauri::LogicalSize;
                    let width = if saved.width.is_finite() {
                        saved.width.clamp(MIN_WINDOW_WIDTH, 4096.0)
                    } else {
                        DEFAULT_WINDOW_WIDTH
                    };
                    let height = if saved.height.is_finite() {
                        saved.height.clamp(MIN_WINDOW_HEIGHT, 2160.0)
                    } else {
                        DEFAULT_WINDOW_HEIGHT
                    };
                    let _ = window.set_size(LogicalSize { width, height });
                }
            }

            #[cfg(target_os = "windows")]
            if let Ok(scale_factor) = window.scale_factor() {
                apply_windows_dpi_zoom(&window, scale_factor);
            }

            let save_path = state_path.clone();
            let window_app = window.app_handle().clone();
            window.on_window_event(move |event| match event {
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    if let Some(window) = window_app.get_webview_window(MAIN_WINDOW_LABEL) {
                        let _ = window.hide();
                    }
                }
                WindowEvent::Resized(size) => {
                    if let Ok(json) = serde_json::to_string(&WindowSizeState {
                        width: size.width as f64,
                        height: size.height as f64,
                    }) {
                        let _ = std::fs::write(&save_path, json);
                    }
                }
                #[cfg(target_os = "windows")]
                WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                    if let Some(window) = window_app.get_webview_window(MAIN_WINDOW_LABEL) {
                        apply_windows_dpi_zoom(&window, *scale_factor);
                    }
                }
                #[cfg(not(target_os = "windows"))]
                WindowEvent::ScaleFactorChanged { .. } => {}
                #[cfg(not(target_os = "windows"))]
                WindowEvent::DragDrop(DragDropEvent::Enter { position, .. })
                | WindowEvent::DragDrop(DragDropEvent::Over { position, .. }) => {
                    tracing::debug!(?position, "[drag-drop] native DragDrop enter/over");
                    let _ = window_app
                        .emit("native-drag-state", NativeDragStateEvent { dragging: true });
                }
                #[cfg(not(target_os = "windows"))]
                WindowEvent::DragDrop(DragDropEvent::Leave) => {
                    tracing::debug!("[drag-drop] native DragDrop leave");
                    let _ = window_app.emit(
                        "native-drag-state",
                        NativeDragStateEvent { dragging: false },
                    );
                }
                #[cfg(not(target_os = "windows"))]
                WindowEvent::DragDrop(DragDropEvent::Drop { paths, position }) => {
                    tracing::info!(
                        count = paths.len(),
                        ?paths,
                        ?position,
                        "[drag-drop] native DragDrop drop"
                    );
                    if paths.is_empty() {
                        tracing::warn!("[drag-drop] native drop received empty paths");
                        return;
                    }
                    let paths: Vec<String> = paths
                        .iter()
                        .map(|path| path.to_string_lossy().into_owned())
                        .collect();
                    tracing::info!(
                        ?paths,
                        "[drag-drop] storing dropped files for frontend poll"
                    );
                    let state = window_app.state::<AppState>();
                    state.push_dropped_files(paths);
                }
                _ => {}
            });

            info!(
                "[setup] Tauri setup complete in {}ms",
                setup_start.elapsed().as_millis()
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            window_start_dragging,
            window_minimize,
            window_toggle_maximize,
            window_close,
            window_get_position,
            window_set_position,
            app_health,
            get_app_version,
            frontend_heartbeat,
            import_files,
            pick_invoice_files,
            pick_any_files,
            pick_save_file,
            poll_dropped_files,
            import_dropped_file,
            list_import_jobs,
            save_invoice_extraction,
            list_invoices,
            search_invoices,
            get_tag_options,
            get_invoice_detail,
            mark_invoice_viewed,
            count_unviewed_invoices,
            open_invoice_raw_file_in_browser,
            update_invoice,
            update_invoice_items,
            batch_update_invoices,
            batch_delete_invoices,
            check_invoice_duplicates,
            resolve_duplicate,
            regenerate_all_duplicates,
            export_invoices,
            merge_invoices,
            export_pdf_report,
            recognize_raw_file,
            test_llm_connection,
            analyze_email_error,
            get_dashboard_stats,
            add_watch_dir,
            remove_watch_dir,
            list_watch_dirs,
            update_watch_dir,
            toggle_watch_dir,
            add_email_source,
            update_email_source,
            remove_email_source,
            list_email_sources,
            toggle_email_source,
            sync_email_source,
            sync_all_email_sources,
            test_email_connection,
            set_chroma_config,
            get_chroma_config,
            set_embedding_enabled,
            get_embedding_status,
            download_embedding_model,
            set_badge_config,
            get_badge_config,
            get_theme,
            set_theme,
            set_invoice_badge,
            test_chroma_connection,
            test_embedding_connection,
            regenerate_all_embeddings,
            search_invoices_semantic,
            create_agent_session,
            list_agent_sessions,
            get_agent_session,
            delete_agent_session,
            update_agent_session_title,
            send_agent_message,
            send_agent_message_stream,
            attach_agent_file,
            list_agent_attachments,
            remove_agent_attachment,
            list_agent_tasks,
            list_agent_artifacts,
            open_agent_artifact_file,
            open_agent_artifact_folder,
            delete_agent_artifact,
            move_export_file,
            confirm_agent_action,
            confirm_agent_action_stream,
            generate_session_title,
            list_events,
            get_unread_event_count,
            get_unread_failed_import_event_count,
            mark_event_read,
            mark_all_events_read,
            set_llm_config,
            get_llm_config,
            set_llm_audit_enabled,
            get_llm_audit_enabled,
            get_recognition_queue_status,
            raw_file_has_invoices,
            get_invoice_id_by_raw_file,
            delete_all_events,
            delete_import_job,
            export_logs,
            export_backup,
            cleanup_storage,
            get_llm_usage,
            get_price_config,
            set_price_config,
            check_external_dependencies,
            get_diagnostic_config,
            set_diagnostic_config,
            run_llm_diagnostic,
            get_log_level,
            set_log_level
        ])
        .run(tauri::generate_context!())
        .expect("failed to run InvoiceVault");
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let workbench = MenuItem::with_id(app, TRAY_WORKBENCH_ID, "工作台", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let version = MenuItem::with_id(
        app,
        TRAY_VERSION_ID,
        format!("版本 {}", env!("GIT_VERSION")),
        false,
        None::<&str>,
    )?;
    let quit_separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, TRAY_QUIT_ID, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&workbench, &separator, &version, &quit_separator, &quit],
    )?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tauri::include_image!("icons/tray.png"))
        .tooltip(format!(
            "{} {}",
            app.package_info().name,
            env!("GIT_VERSION")
        ))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            if event.id() == TRAY_WORKBENCH_ID {
                restore_main_window(app);
            } else if event.id() == TRAY_QUIT_ID {
                app.exit(0);
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = event
            {
                restore_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn claim_single_instance() -> Option<SingleInstanceGuard> {
    let lock_path = single_instance_lock_path();
    if let Some(addr) = read_single_instance_lock(&lock_path) {
        if notify_existing_instance(addr) {
            return None;
        }
        let _ = std::fs::remove_file(&lock_path);
    }

    let listener = match TcpListener::bind(SINGLE_INSTANCE_BIND_ADDR) {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("Failed to create InvoiceVault single-instance listener: {err}");
            return None;
        }
    };
    let addr = match listener.local_addr() {
        Ok(addr) => addr,
        Err(err) => {
            eprintln!("Failed to read InvoiceVault single-instance listener address: {err}");
            return None;
        }
    };
    if let Some(parent) = lock_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(err) = std::fs::write(&lock_path, addr.port().to_string()) {
        eprintln!("Failed to write InvoiceVault single-instance lock: {err}");
        return None;
    }

    Some(SingleInstanceGuard {
        listener: Some(listener),
        lock_path,
    })
}

fn single_instance_lock_path() -> std::path::PathBuf {
    dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("InvoiceVault")
        .join(SINGLE_INSTANCE_LOCK_FILE)
}

fn read_single_instance_lock(path: &Path) -> Option<SocketAddr> {
    let port = std::fs::read_to_string(path)
        .ok()?
        .trim()
        .parse::<u16>()
        .ok()?;
    Some(SocketAddr::from(([127, 0, 0, 1], port)))
}

fn notify_existing_instance(addr: SocketAddr) -> bool {
    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(SINGLE_INSTANCE_TCP_TIMEOUT_MS)) {
        Ok(stream) => stream,
        Err(_) => return false,
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(SINGLE_INSTANCE_TCP_TIMEOUT_MS)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(SINGLE_INSTANCE_TCP_TIMEOUT_MS)));
    if stream.write_all(SINGLE_INSTANCE_SHOW_MESSAGE).is_err() {
        return false;
    }
    let mut response = [0_u8; 32];
    match stream.read(&mut response) {
        Ok(n) => &response[..n] == SINGLE_INSTANCE_OK_MESSAGE,
        Err(_) => false,
    }
}

fn setup_single_instance_listener(listener: TcpListener, app: AppHandle) {
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let mut request = [0_u8; 64];
                    let n = stream.read(&mut request).unwrap_or(0);
                    let message = &request[..n];
                    if message == SINGLE_INSTANCE_SHOW_MESSAGE {
                        restore_main_window(&app);
                        let _ = stream.write_all(SINGLE_INSTANCE_OK_MESSAGE);
                    } else if message == SINGLE_INSTANCE_PING_MESSAGE {
                        let _ = stream.write_all(SINGLE_INSTANCE_OK_MESSAGE);
                    }
                }
                Err(err) => {
                    warn!("Single-instance listener failed: {err}");
                    break;
                }
            }
        }
    });
}

fn restore_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

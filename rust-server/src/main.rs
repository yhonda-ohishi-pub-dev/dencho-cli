use axum::{
    extract::Json as ExtractJson,
    http::{Method, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

const SERVICE_NAME: &str = "dencho-cli";
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

#[derive(Serialize, Deserialize)]
struct DownloadRequest {
    #[serde(rename = "githubUsername")]
    github_username: Option<String>,
    #[serde(rename = "githubPassword")]
    github_password: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct DownloadResponse {
    status: String,
    message: String,
}

/// アプリケーションルートディレクトリを検出
fn get_application_root() -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("実行ファイルパス取得失敗: {}", e))?;

    let exe_dir = exe_path
        .parent()
        .ok_or("実行ファイルディレクトリ取得失敗")?;

    // bin/サブディレクトリにいるかチェック（インストールモード）
    if exe_dir.file_name() == Some(std::ffi::OsStr::new("bin")) {
        let app_root = exe_dir.parent().ok_or("アプリケーションルート取得失敗")?;

        if app_root.join("package.json").exists() {
            return Ok(app_root.to_path_buf());
        }
    }

    // 開発モード: カレントディレクトリにフォールバック
    let cwd = std::env::current_dir()
        .map_err(|e| format!("カレントディレクトリ取得失敗: {}", e))?;

    Ok(cwd)
}

// Windows サービス定義
define_windows_service!(ffi_service_main, service_main);

fn service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service() {
        log_to_file(&format!("サービスエラー: {}", e));
    }
}

fn run_service() -> Result<(), Box<dyn std::error::Error>> {
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                shutdown_tx.send(()).unwrap();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    log_to_file("サービス開始");

    // 非同期ランタイムを作成してサーバーを起動
    let rt = tokio::runtime::Runtime::new()?;

    rt.block_on(async {
        // 環境チェック
        if let Err(e) = check_and_setup_environment() {
            log_to_file(&format!("環境セットアップエラー: {}", e));
            return;
        }

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers(Any);

        let app = Router::new()
            .route("/health", get(health_check))
            .route("/api/download", post(download_invoice))
            .layer(cors);

        let addr = "127.0.0.1:3939";
        log_to_file(&format!("サーバー起動: http://{}", addr));

        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

        // シャットダウン監視タスク
        let shutdown_signal = async move {
            loop {
                if shutdown_rx.try_recv().is_ok() {
                    log_to_file("シャットダウン信号受信");
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        };

        tokio::select! {
            _ = axum::serve(listener, app) => {}
            _ = shutdown_signal => {}
        }
    });

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    log_to_file("サービス停止");
    Ok(())
}

fn log_to_file(message: &str) {
    let log_dir = get_application_root()
        .map(|p| p.join("logs"))
        .unwrap_or_else(|_| PathBuf::from("C:\\ProgramData\\dencho-cli\\logs"));

    let _ = std::fs::create_dir_all(&log_dir);
    let log_file = log_dir.join("service.log");

    let timestamp = chrono_lite_timestamp();
    let log_line = format!("[{}] {}\n", timestamp, message);

    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .and_then(|mut f| std::io::Write::write_all(&mut f, log_line.as_bytes()));
}

fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}", now)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "install" => {
                install_service();
                return;
            }
            "uninstall" => {
                uninstall_service();
                return;
            }
            "run" => {
                // コンソールモードで実行
                run_console_mode();
                return;
            }
            _ => {}
        }
    }

    // サービスとして起動
    if let Err(e) = service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
        // サービスとして起動できない場合（コンソールから直接実行）
        eprintln!("サービスとして起動できません: {}", e);
        eprintln!("コンソールモードで実行するには: dencho-cli.exe run");
        eprintln!("サービスとしてインストールするには: dencho-cli.exe install");
    }
}

fn run_console_mode() {
    println!("=== dencho-cli サーバー (コンソールモード) ===");

    if let Err(e) = check_and_setup_environment() {
        eprintln!("❌ 環境セットアップエラー: {}", e);
        std::process::exit(1);
    }

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers(Any);

        let app = Router::new()
            .route("/health", get(health_check))
            .route("/api/download", post(download_invoice))
            .layer(cors);

        let addr = "127.0.0.1:3939";
        println!("✓ サーバー起動完了: http://{}", addr);
        println!("  Ctrl+C で終了します\n");

        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });
}

fn install_service() {
    println!("サービスをインストール中...");

    let exe_path = std::env::current_exe().expect("実行ファイルパス取得失敗");

    let output = Command::new("sc")
        .args([
            "create",
            SERVICE_NAME,
            &format!("binPath={}", exe_path.display()),
            "start=auto",
            "DisplayName=Dencho CLI Server",
        ])
        .output();

    match output {
        Ok(result) if result.status.success() => {
            println!("✓ サービスインストール完了");
            println!("  サービス開始: sc start {}", SERVICE_NAME);

            // サービスを開始
            let _ = Command::new("sc").args(["start", SERVICE_NAME]).status();
            println!("✓ サービスを開始しました");
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            eprintln!("❌ インストール失敗: {}", stderr);
            eprintln!("管理者権限で実行してください");
        }
        Err(e) => {
            eprintln!("❌ sc コマンド実行エラー: {}", e);
        }
    }
}

fn uninstall_service() {
    println!("サービスをアンインストール中...");

    // まずサービスを停止
    let _ = Command::new("sc").args(["stop", SERVICE_NAME]).status();

    let output = Command::new("sc")
        .args(["delete", SERVICE_NAME])
        .output();

    match output {
        Ok(result) if result.status.success() => {
            println!("✓ サービスアンインストール完了");
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            eprintln!("❌ アンインストール失敗: {}", stderr);
        }
        Err(e) => {
            eprintln!("❌ sc コマンド実行エラー: {}", e);
        }
    }
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn download_invoice(
    ExtractJson(payload): ExtractJson<DownloadRequest>,
) -> (StatusCode, Json<DownloadResponse>) {
    log_to_file("ダウンロードリクエスト受信");

    let app_root = match get_application_root() {
        Ok(path) => path,
        Err(e) => {
            log_to_file(&format!("アプリケーションルート取得エラー: {}", e));
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DownloadResponse {
                    status: "error".to_string(),
                    message: format!("環境設定エラー: {}", e),
                }),
            );
        }
    };

    let script_path = app_root.join("dist").join("download-supabase-invoice.js");

    if !script_path.exists() {
        log_to_file(&format!("スクリプトが見つかりません: {}", script_path.display()));
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(DownloadResponse {
                status: "error".to_string(),
                message: format!("スクリプトファイルが見つかりません: {}", script_path.display()),
            }),
        );
    }

    let mut cmd = Command::new("node");
    cmd.arg(&script_path).current_dir(&app_root);

    // Playwright ブラウザパスを設定
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| {
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
    });
    let browsers_path = std::path::Path::new(&appdata)
        .join("dencho-cli")
        .join("browsers");
    cmd.env("PLAYWRIGHT_BROWSERS_PATH", &browsers_path);

    if let Some(username) = payload.github_username {
        if !username.is_empty() {
            cmd.env("GITHUB_USERNAME", username);
        }
    }
    if let Some(password) = payload.github_password {
        if !password.is_empty() {
            cmd.env("GITHUB_PASSWORD", password);
        }
    }

    let output = cmd.output();

    match output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let stderr = String::from_utf8_lossy(&result.stderr);

            if result.status.success() {
                log_to_file("ダウンロード成功");
                (
                    StatusCode::OK,
                    Json(DownloadResponse {
                        status: "success".to_string(),
                        message: "Supabase 請求書のダウンロードが完了しました".to_string(),
                    }),
                )
            } else {
                log_to_file(&format!("ダウンロード失敗: {} {}", stdout, stderr));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(DownloadResponse {
                        status: "error".to_string(),
                        message: format!("ダウンロードエラー: {}", stderr.trim()),
                    }),
                )
            }
        }
        Err(e) => {
            log_to_file(&format!("Node.js 実行エラー: {}", e));
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DownloadResponse {
                    status: "error".to_string(),
                    message: format!("Node.js 実行エラー: {}", e),
                }),
            )
        }
    }
}

fn check_and_setup_environment() -> Result<(), String> {
    let app_root = get_application_root()?;

    // Node.js チェック
    let node_check = Command::new("node").arg("--version").output();
    if node_check.is_err() || !node_check.unwrap().status.success() {
        return Err("Node.js が見つかりません".to_string());
    }

    // node_modules チェック
    let node_modules_path = app_root.join("node_modules");
    if !node_modules_path.exists() {
        let npm_cmd = if cfg!(target_os = "windows") {
            "npm.cmd"
        } else {
            "npm"
        };
        let status = Command::new(npm_cmd)
            .arg("install")
            .current_dir(&app_root)
            .status();

        if status.is_err() || !status.unwrap().success() {
            return Err("npm install に失敗しました".to_string());
        }
    }

    // Playwright ブラウザチェック
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let browsers_path = std::path::Path::new(&appdata)
        .join("dencho-cli")
        .join("browsers");

    if !browsers_path.exists()
        || std::fs::read_dir(&browsers_path)
            .ok()
            .map_or(true, |mut d| d.next().is_none())
    {
        let npx_cmd = if cfg!(target_os = "windows") {
            "npx.cmd"
        } else {
            "npx"
        };
        let status = Command::new(npx_cmd)
            .args(["playwright", "install", "chromium"])
            .current_dir(&app_root)
            .env("PLAYWRIGHT_BROWSERS_PATH", &browsers_path)
            .status();

        if status.is_err() || !status.unwrap().success() {
            return Err("Playwright ブラウザのインストールに失敗しました".to_string());
        }
    }

    Ok(())
}

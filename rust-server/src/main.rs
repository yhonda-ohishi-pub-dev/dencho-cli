use axum::{
    extract::Json as ExtractJson,
    http::{Method, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use tower_http::cors::{Any, CorsLayer};

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
            println!("📦 インストール場所から実行: {}", app_root.display());
            return Ok(app_root.to_path_buf());
        }
    }

    // 開発モード: カレントディレクトリにフォールバック
    let cwd = std::env::current_dir()
        .map_err(|e| format!("カレントディレクトリ取得失敗: {}", e))?;

    println!("🔧 開発ディレクトリから実行: {}", cwd.display());
    Ok(cwd)
}

fn log_to_file(message: &str) {
    let log_dir = get_application_root()
        .map(|p| p.join("logs"))
        .unwrap_or_else(|_| PathBuf::from("."));

    let _ = std::fs::create_dir_all(&log_dir);
    let log_file = log_dir.join("server.log");

    let timestamp = chrono_lite_timestamp();
    let log_line = format!("[{}] {}\n", timestamp, message);

    // コンソールにも出力
    print!("{}", log_line);

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

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    // "run" 引数があってもなくても同じ動作（互換性のため）
    if args.len() > 1 && args[1] != "run" {
        println!("使用方法: dencho-cli.exe [run]");
        println!("  run  サーバーを起動します（デフォルト）");
        return;
    }

    println!("=== dencho-cli サーバー ===");

    if let Err(e) = check_and_setup_environment() {
        eprintln!("❌ 環境セットアップエラー: {}", e);
        std::process::exit(1);
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
    println!("✓ サーバー起動完了: http://{}", addr);
    println!("  ウィンドウを閉じるとサーバーが停止します\n");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
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
    println!("🔍 環境チェック中...");

    let app_root = get_application_root()?;

    // Node.js チェック
    println!("  [1/3] Node.js チェック...");
    let node_check = Command::new("node").arg("--version").output();
    match node_check {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            println!("    ✓ Node.js: {}", version.trim());
        }
        _ => return Err("Node.js が見つかりません".to_string()),
    }

    // node_modules チェック
    println!("  [2/3] 依存関係チェック...");
    let node_modules_path = app_root.join("node_modules");
    if !node_modules_path.exists() {
        println!("    ⚙ npm install を実行中...");
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
        println!("    ✓ npm install 完了");
    } else {
        println!("    ✓ node_modules 存在確認");
    }

    // Playwright ブラウザチェック
    println!("  [3/3] Playwright ブラウザチェック...");
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let browsers_path = std::path::Path::new(&appdata)
        .join("dencho-cli")
        .join("browsers");

    if !browsers_path.exists()
        || std::fs::read_dir(&browsers_path)
            .ok()
            .map_or(true, |mut d| d.next().is_none())
    {
        println!("    ⚙ Playwright ブラウザをダウンロード中...");
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
        println!("    ✓ Playwright ブラウザインストール完了");
    } else {
        println!("    ✓ Playwright ブラウザ存在確認");
    }

    println!("✓ 環境チェック完了\n");
    Ok(())
}

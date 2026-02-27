use axum::{
    http::{Method, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::process::Command;
use tower_http::cors::{Any, CorsLayer};

#[derive(Serialize, Deserialize)]
struct DownloadResponse {
    status: String,
    message: String,
}

#[tokio::main]
async fn main() {
    // 初回起動チェック
    println!("=== dencho-cli サーバー起動中 ===");

    if let Err(e) = check_and_setup_environment() {
        eprintln!("❌ 環境セットアップエラー: {}", e);
        std::process::exit(1);
    }

    // CORS設定
    let cors = CorsLayer::new()
        .allow_origin(Any)  // 開発時は全て許可。本番では GitHub Pages のオリジンのみ許可すべき
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    // ルーター設定
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/download", post(download_invoice))
        .layer(cors);

    // サーバー起動
    let addr = "127.0.0.1:3939";
    println!("✓ サーバー起動完了: http://{}", addr);
    println!("  GitHub Pages から POST http://localhost:3939/api/download で呼び出してください");
    println!("  Ctrl+C で終了します\n");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// ヘルスチェックエンドポイント
async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

/// 請求書ダウンロードエンドポイント
async fn download_invoice() -> (StatusCode, Json<DownloadResponse>) {
    println!("📥 ダウンロードリクエスト受信");

    // Node.js スクリプトを実行
    let output = Command::new("node")
        .arg("dist/download-supabase-invoice.js")
        .output();

    match output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let stderr = String::from_utf8_lossy(&result.stderr);

            if result.status.success() {
                println!("✓ ダウンロード成功");
                if !stdout.is_empty() {
                    println!("  出力: {}", stdout.trim());
                }
                (
                    StatusCode::OK,
                    Json(DownloadResponse {
                        status: "success".to_string(),
                        message: "Supabase 請求書のダウンロードが完了しました".to_string(),
                    }),
                )
            } else {
                eprintln!("❌ ダウンロード失敗");
                eprintln!("  stdout: {}", stdout);
                eprintln!("  stderr: {}", stderr);
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
            eprintln!("❌ Node.js 実行エラー: {}", e);
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

/// 環境チェックとセットアップ
fn check_and_setup_environment() -> Result<(), String> {
    println!("🔍 環境チェック中...");

    // 1. Node.js インストール確認
    println!("  [1/3] Node.js インストール確認...");
    let node_check = Command::new("node").arg("--version").output();

    match node_check {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            println!("    ✓ Node.js: {}", version.trim());
        }
        _ => {
            return Err(
                "Node.js が見つかりません。https://nodejs.org/ からインストールしてください"
                    .to_string(),
            );
        }
    }

    // 2. node_modules 存在確認
    println!("  [2/3] 依存関係チェック...");
    if !std::path::Path::new("node_modules").exists() {
        println!("    ⚙ npm install を実行中...");
        let npm_install = Command::new("npm").arg("install").status();

        match npm_install {
            Ok(status) if status.success() => {
                println!("    ✓ npm install 完了");
            }
            _ => {
                return Err("npm install に失敗しました".to_string());
            }
        }
    } else {
        println!("    ✓ node_modules 存在確認");
    }

    // 3. Playwright ブラウザ確認
    println!("  [3/3] Playwright ブラウザチェック...");

    // %APPDATA%\dencho-cli\browsers をチェック
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| {
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
    });
    let browsers_path = std::path::Path::new(&appdata).join("dencho-cli").join("browsers");

    if !browsers_path.exists() || std::fs::read_dir(&browsers_path).ok().map_or(true, |mut d| d.next().is_none()) {
        println!("    ⚙ Playwright ブラウザをダウンロード中 (約 300MB, 1-2分)...");

        // PLAYWRIGHT_BROWSERS_PATH を設定
        let mut cmd = Command::new("npx");
        cmd.arg("playwright")
            .arg("install")
            .arg("chromium")
            .env("PLAYWRIGHT_BROWSERS_PATH", &browsers_path);

        let status = cmd.status();

        match status {
            Ok(s) if s.success() => {
                println!("    ✓ Playwright ブラウザインストール完了");
            }
            _ => {
                return Err("Playwright ブラウザのインストールに失敗しました".to_string());
            }
        }
    } else {
        println!("    ✓ Playwright ブラウザ存在確認");
    }

    println!("✓ 環境チェック完了\n");
    Ok(())
}

# dencho-cli

Supabase 請求書を GitHub Pages から自動ダウンロードする Windows アプリケーション

## 概要

このツールは以下の2つのコンポーネントで構成されています:

1. **Rust HTTP サーバー** (`dencho-cli.exe`) - localhost:3939 で待機し、Playwright CLI を実行
2. **既存の Playwright CLI** (TypeScript) - Supabase ダッシュボードから請求書を自動ダウンロード

GitHub Pages にデプロイされている `denchoho-invoice` アプリから、ローカルで動作する `dencho-cli.exe` に POST リクエストを送信して請求書をダウンロードできます。

## 必要な環境

- Windows 10/11
- Node.js 18 以上 ([公式サイト](https://nodejs.org/)からダウンロード)

## インストール

### 方法1: GitHub Releases からダウンロード (推奨)

1. [GitHub Releases](https://github.com/YOUR_USERNAME/dencho-cli/releases) から最新の `dencho-cli-windows.zip` をダウンロード
2. ZIP を解凍して任意のフォルダに配置
3. `dencho-cli.exe` をダブルクリックして起動

### 方法2: ソースからビルド

```bash
# リポジトリをクローン
git clone https://github.com/YOUR_USERNAME/dencho-cli.git
cd dencho-cli

# Node.js 依存関係をインストール
npm install

# TypeScript をビルド
npm run build

# Rust サーバーをビルド
cd rust-server
cargo build --release

# 実行
cd ..
rust-server/target/release/dencho-cli.exe
```

## 初回起動

`dencho-cli.exe` を初めて起動すると、以下が自動実行されます:

1. **Node.js バージョンチェック** - Node.js がインストールされているか確認
2. **依存関係インストール** - `npm install` を実行 (初回のみ)
3. **Playwright ブラウザダウンロード** - Chromium ブラウザをダウンロード (約 300MB, 1-2分)

完了すると `http://localhost:3939` でサーバーが起動します。

```
=== dencho-cli サーバー起動中 ===
🔍 環境チェック中...
  [1/3] Node.js インストール確認...
    ✓ Node.js: v18.x.x
  [2/3] 依存関係チェック...
    ✓ node_modules 存在確認
  [3/3] Playwright ブラウザチェック...
    ✓ Playwright ブラウザ存在確認
✓ 環境チェック完了

✓ サーバー起動完了: http://127.0.0.1:3939
  GitHub Pages から POST http://localhost:3939/api/download で呼び出してください
  Ctrl+C で終了します
```

## 使い方

### 1. サーバーを起動

`dencho-cli.exe` をダブルクリックして起動します。コンソールウィンドウが開き、サーバーが起動します。

### 2. GitHub Pages から呼び出し

ブラウザで [denchoho-invoice](https://username.github.io/denchoho-invoice/) を開き、「Supabase請求書」ボタンをクリックします。

### 3. 認証 (初回のみ)

初回実行時に GitHub OAuth でログインが必要です。ブラウザウィンドウが自動的に開くので、ログインしてください。

### 4. ダウンロード完了

請求書が自動的にダウンロードされ、`downloads/invoice/` フォルダに保存されます。

ファイル名形式: `supabase-invoice-YYYY-MM-DD.pdf`

## API エンドポイント

### GET /health

ヘルスチェックエンドポイント。サーバーが起動しているか確認できます。

```bash
curl http://localhost:3939/health
```

レスポンス:
```json
{"status":"ok"}
```

### POST /api/download

Supabase 請求書をダウンロードします。

```bash
curl -X POST http://localhost:3939/api/download
```

成功時のレスポンス:
```json
{
  "status": "success",
  "message": "Supabase 請求書のダウンロードが完了しました"
}
```

エラー時のレスポンス:
```json
{
  "status": "error",
  "message": "エラーの詳細"
}
```

## トラブルシューティング

### 「dencho-cli.exe が起動していません」エラー

→ `dencho-cli.exe` をダブルクリックして先に起動してください

### ブラウザダウンロードに失敗する

→ インターネット接続を確認してください。ファイアウォールが npx をブロックしている可能性があります。

### Node.js が見つからない

→ [Node.js 公式サイト](https://nodejs.org/) からインストールしてください (LTS 版を推奨)

### ポート 3939 が使用中

→ 他のアプリケーションがポート 3939 を使用している可能性があります。そのアプリを終了してから再度起動してください。

### CORS エラー

→ ローカル開発時は問題ありませんが、本番環境では GitHub Pages のオリジンを `main.rs` で明示的に許可する必要があります。

## 開発

### ローカルでビルド

```bash
# TypeScript をビルド
npm run build

# Rust サーバーをビルド (開発モード)
cd rust-server
cargo run

# Rust サーバーをビルド (リリースモード)
cargo build --release
```

### ディレクトリ構成

```
dencho-cli/
├── rust-server/           # Rust HTTP サーバー
│   ├── src/
│   │   └── main.rs       # メインサーバー実装
│   └── Cargo.toml        # Rust 依存関係
├── src/                   # TypeScript Playwright CLI
│   └── download-supabase-invoice.ts
├── dist/                  # ビルド成果物
├── node_modules/          # Node.js 依存関係
├── downloads/invoice/     # ダウンロードされた請求書
├── .auth/                 # 認証情報 (gitignore)
└── package.json
```

## セキュリティ

- 認証情報は `.auth/supabase-state.json` に保存されます (`.gitignore` に含まれています)
- CORS は開発時は全オリジン許可していますが、本番環境では特定のオリジンのみ許可すべきです
- localhost:3939 は外部からアクセスできません (127.0.0.1 にバインド)

## ライセンス

ISC

## 作者

[YOUR_NAME]

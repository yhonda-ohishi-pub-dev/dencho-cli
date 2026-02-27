# dencho-cli

Supabase請求書ダウンロードツール

## プロジェクト構成

- **TypeScript (Playwright)**: `src/` - 請求書ダウンロードスクリプト
- **Rust Server**: `rust-server/` - HTTPサーバー
- **Nuxt App**: `denchoho-invoice/` - フロントエンドアプリ

## バージョン管理

- **npm** (`package.json`): TypeScript/Playwrightのバージョン
- **Cargo** (`rust-server/Cargo.toml`): Rustサーバーのバージョン

npmとCargoのバージョンは独立して管理する。揃える必要はない。

## ビルド

```bash
npm run build          # TypeScriptをビルド
cargo build --release  # Rustサーバーをビルド (rust-server/)
```

## リリース手順

1. コードを修正
2. `npm run build` でビルド確認
3. `rust-server/Cargo.toml` の version をパッチ
4. コミット & プッシュ → **自動でタグ作成 & リリース**

```bash
git add -A && git commit -m "v1.0.xx" && git push
```

※ Cargo.tomlのバージョンが変わっていれば、GitHub Actionsが自動でタグを作成してリリースビルドを実行する

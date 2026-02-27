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
3. 必要に応じて各パッケージのバージョンをパッチ
   - npm: `package.json` の version
   - Cargo: `rust-server/Cargo.toml` の version
4. コミット & プッシュ
5. **タグを作成してプッシュ**（GitHub Actionsでリリースビルドがトリガーされる）
   ```bash
   git tag v1.0.xx
   git push origin v1.0.xx
   ```

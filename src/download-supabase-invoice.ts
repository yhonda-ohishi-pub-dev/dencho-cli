import { chromium } from '@playwright/test';
import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const AUTH_STATE_PATH = path.join(process.cwd(), '.auth', 'supabase-state.json');
const DOWNLOAD_DIR = path.join(process.cwd(), 'downloads', 'invoice');

async function downloadSupabaseInvoices() {
  // ダウンロードディレクトリ作成
  if (!fs.existsSync(DOWNLOAD_DIR)) {
    fs.mkdirSync(DOWNLOAD_DIR, { recursive: true });
  }

  // 認証状態の確認
  const hasAuth = fs.existsSync(AUTH_STATE_PATH);

  const browser = await chromium.launch({
    headless: false // デバッグのため一旦headedモード
  });

  const context = hasAuth
    ? await browser.newContext({ storageState: AUTH_STATE_PATH })
    : await browser.newContext();

  // ダウンロード設定
  await context.setExtraHTTPHeaders({
    'Accept-Language': 'ja-JP,ja;q=0.9,en-US;q=0.8,en;q=0.7'
  });

  const page = await context.newPage();

  try {
    console.log(hasAuth ? '保存済みの認証情報を使用します' : '初回実行: 手動でログインしてください...');

    // 組織ページに移動（未ログインの場合は自動的にログインページにリダイレクトされる）
    await page.goto('https://supabase.com/dashboard/organizations', {
      waitUntil: 'domcontentloaded',
      timeout: 60000
    });
    await page.waitForTimeout(3000);

    // ログインページにリダイレクトされたかチェック
    const currentUrl = page.url();
    console.log('現在のURL:', currentUrl);

    if (currentUrl.includes('/sign-in')) {
      console.log('ログインが必要です。GitHub認証ボタンを探しています...');

      // ボタンが表示されるまで待機
      await page.waitForSelector('button:has-text("Continue with GitHub")', { timeout: 10000 });
      console.log('GitHub認証ボタンが見つかりました。クリックします...');

      // GitHub OAuthボタンをクリック
      await page.click('button:has-text("Continue with GitHub")');

      // GitHubログイン画面に遷移するか、直接passkey画面に行くか待機
      await page.waitForTimeout(3000);
      const afterClickUrl = page.url();
      console.log('クリック後のURL:', afterClickUrl);

      // 既にorganizationsページに戻っている場合（認証不要）
      if (afterClickUrl.includes('/organizations')) {
        console.log('認証が完了しました（GitHubセッション利用）');
      }
      // GitHubのログインページに遷移した場合（パスワード入力が必要）
      else if (afterClickUrl.includes('github.com') && afterClickUrl.includes('login')) {
        console.log('GitHubログインページに遷移しました');
        console.log('Username/Passwordを入力してください...');
        console.log('その後、passkey認証を完了してください...');

        // 組織ページに戻るまで待機
        console.log('認証完了を待機中...');
        await page.waitForURL('**/organizations', { timeout: 300000 });
        console.log('組織ページに戻りました');
      }
      // passkey画面に遷移した場合
      else {
        console.log('passkey認証を完了してください...');

        // 組織ページに戻るまで待機
        console.log('認証完了を待機中...');
        await page.waitForURL('**/organizations', { timeout: 300000 });
        console.log('組織ページに戻りました');
      }

      // 認証状態を保存
      await context.storageState({ path: AUTH_STATE_PATH });
      console.log('認証情報を保存しました: ' + AUTH_STATE_PATH);
    } else {
      console.log('既にログイン済みです');
    }

    // ページが完全に読み込まれるまで待機
    await page.waitForTimeout(3000);

    // 組織選択
    console.log('組織を選択中...');
    console.log('現在のページURL:', page.url());

    // 組織リンクが表示されるまで待機
    await page.waitForSelector('a:has-text("yhonda-ohishi\'s Org")', { timeout: 10000 });
    console.log('組織リンクが見つかりました');

    await page.click('a:has-text("yhonda-ohishi\'s Org")');

    // 組織ページが読み込まれるまで待機
    await page.waitForTimeout(2000);

    // Billingページへ移動
    console.log('Billingページへ移動中...');
    await page.getByRole('link', { name: 'Billing' }).click();
    await page.waitForTimeout(2000);

    // ダウンロード処理
    console.log('請求書をダウンロード中...');

    const downloadPromise = page.waitForEvent('download');
    await page.locator('.relative.justify-center.cursor-pointer.inline-flex.items-center.space-x-2.text-center.font-regular.ease-out.duration-200.rounded-md.outline-none.transition-all.outline-0.focus-visible\\:outline-4.focus-visible\\:outline-offset-1.border.text-foreground.bg-transparent').first().click();
    const download = await downloadPromise;

    // ファイル名を生成（日付付き）
    const timestamp = new Date().toISOString().split('T')[0];
    const filename = `supabase-invoice-${timestamp}.pdf`;
    const filepath = path.join(DOWNLOAD_DIR, filename);

    await download.saveAs(filepath);
    console.log(`✓ ダウンロード完了: ${filepath}`);

  } catch (error) {
    console.error('エラーが発生しました:', error);
    throw error;
  } finally {
    await browser.close();
  }
}

// 実行
downloadSupabaseInvoices()
  .then(() => {
    console.log('処理が完了しました');
    process.exit(0);
  })
  .catch((error) => {
    console.error('処理に失敗しました:', error);
    process.exit(1);
  });

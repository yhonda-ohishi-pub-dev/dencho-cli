import { chromium } from '@playwright/test';
import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const AUTH_STATE_PATH = path.join(process.cwd(), '.auth', 'supabase-state.json');
const DOWNLOAD_DIR = path.join(process.cwd(), 'downloads', 'invoice');
const LOG_DIR = path.join(process.cwd(), 'logs');
const LOG_FILE = path.join(LOG_DIR, 'supabase-download.log');

// GitHub認証情報を環境変数から取得
const GITHUB_USERNAME = process.env.GITHUB_USERNAME || '';
const GITHUB_PASSWORD = process.env.GITHUB_PASSWORD || '';

// ログ関数
function log(message: string) {
  const timestamp = new Date().toISOString();
  const logMessage = `[${timestamp}] ${message}`;
  console.log(logMessage);

  // ログディレクトリ作成
  if (!fs.existsSync(LOG_DIR)) {
    fs.mkdirSync(LOG_DIR, { recursive: true });
  }

  // ファイルに追記
  fs.appendFileSync(LOG_FILE, logMessage + '\n');
}

function logError(message: string, error?: unknown) {
  const timestamp = new Date().toISOString();
  const errorDetail = error instanceof Error ? error.stack || error.message : String(error);
  const logMessage = `[${timestamp}] ERROR: ${message}\n${errorDetail}`;
  console.error(logMessage);

  if (!fs.existsSync(LOG_DIR)) {
    fs.mkdirSync(LOG_DIR, { recursive: true });
  }

  fs.appendFileSync(LOG_FILE, logMessage + '\n');
}

async function downloadSupabaseInvoices() {
  // ダウンロードディレクトリ作成
  if (!fs.existsSync(DOWNLOAD_DIR)) {
    fs.mkdirSync(DOWNLOAD_DIR, { recursive: true });
  }

  // 認証状態の確認
  const hasAuth = fs.existsSync(AUTH_STATE_PATH);

  // headlessモード: 認証済みの場合はheadless、未認証の場合はheaded（MFA入力のため）
  // 環境変数 HEADLESS=true で強制headlessモードも可能
  const forceHeadless = process.env.HEADLESS === 'true';
  const headless = forceHeadless || hasAuth;
  log(`ブラウザモード: ${headless ? 'headless' : 'headed'}`);

  const browser = await chromium.launch({
    headless: headless
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
    log(hasAuth ? '保存済みの認証情報を使用します' : '初回実行: 手動でログインしてください...');

    // 組織ページに移動（未ログインの場合は自動的にログインページにリダイレクトされる）
    await page.goto('https://supabase.com/dashboard/organizations', {
      waitUntil: 'domcontentloaded',
      timeout: 60000
    });
    await page.waitForTimeout(3000);

    // ログインページにリダイレクトされたかチェック
    const currentUrl = page.url();
    log(`現在のURL: ${currentUrl}`);

    if (currentUrl.includes('/sign-in')) {
      log('ログインが必要です。GitHub認証ボタンを探しています...');

      // ボタンが表示されるまで待機
      await page.waitForSelector('button:has-text("Continue with GitHub")', { timeout: 10000 });
      log('GitHub認証ボタンが見つかりました。クリックします...');

      // GitHub OAuthボタンをクリック
      await page.click('button:has-text("Continue with GitHub")');

      // GitHubログイン画面に遷移するか、直接passkey画面に行くか待機
      await page.waitForLoadState('networkidle');
      await page.waitForTimeout(2000);
      let afterClickUrl = page.url();
      log(`クリック直後のURL: ${afterClickUrl}`);
      log(`分岐判定: organizations=${afterClickUrl.includes('/organizations')} github.com=${afterClickUrl.includes('github.com')} login=${afterClickUrl.includes('login')}`);

      // 既にorganizationsページに戻っている場合（認証不要）
      if (afterClickUrl.includes('/organizations')) {
        log('認証が完了しました（GitHubセッション利用）');
      }
      // GitHubのログインページに遷移した場合（パスワード入力が必要）
      else if (afterClickUrl.includes('github.com') && afterClickUrl.includes('login')) {
        log('GitHubログインページに遷移しました');

        if (GITHUB_USERNAME && GITHUB_PASSWORD) {
          log('保存された認証情報を使用して自動ログイン中...');

          // Username/Email入力
          await page.fill('input[name="login"]', GITHUB_USERNAME);
          await page.fill('input[name="password"]', GITHUB_PASSWORD);

          // Sign inボタンをクリック
          await page.click('input[type="submit"][value="Sign in"]');

          log('ログイン送信完了。認証を待機中...');
        } else {
          log('認証情報が保存されていません。手動でUsername/Passwordを入力してください...');
          log('その後、passkey認証を完了してください...');
        }

        // 組織ページに戻るまで待機
        log('認証完了を待機中...');
        await page.waitForURL('**/organizations', { timeout: 300000 });
        log('組織ページに戻りました');
      }
      // passkey/2FA画面に遷移した場合、またはその他
      else {
        log('2FA/passkey/OAuth認証フローに入りました...');
        log('organizationsページへのリダイレクトを待機中...');

        // organizationsページに到達するまで待機（最大5分）
        // waitForFunction は現在のURLもチェックするため、既に到達済みでも正常に完了する
        await page.waitForFunction(
          () => {
            const url = window.location.pathname;
            return url.includes('/organizations');
          },
          { timeout: 300000 }
        );
        log('組織ページに到達しました');
      }

      // 認証状態を保存
      await context.storageState({ path: AUTH_STATE_PATH });
      log('認証情報を保存しました: ' + AUTH_STATE_PATH);
    } else {
      log('既にログイン済みです');
    }

    // ページが完全に読み込まれるまで待機
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(3000);

    // 組織選択
    log('組織を選択中...');
    log(`現在のページURL: ${page.url()}`);

    // 組織リンクが表示されるまで待機
    const orgSelector = 'a[href*="/org/"]';
    await page.waitForSelector(orgSelector, { timeout: 30000 });
    log('組織リンクが見つかりました');

    // 最初の組織リンクをクリック
    const orgLink = page.locator(orgSelector).first();
    await orgLink.waitFor({ state: 'visible' });
    log('組織リンクが表示されました。クリックします...');
    await orgLink.click();

    // 組織ページが読み込まれるまで待機
    await page.waitForTimeout(2000);

    // Billingページへ移動
    log('Billingページへ移動中...');
    await page.getByRole('link', { name: 'Billing' }).click();
    await page.waitForTimeout(2000);

    // ダウンロード処理
    log('請求書をダウンロード中...');

    const downloadPromise = page.waitForEvent('download');
    await page.locator('.relative.justify-center.cursor-pointer.inline-flex.items-center.space-x-2.text-center.font-regular.ease-out.duration-200.rounded-md.outline-none.transition-all.outline-0.focus-visible\\:outline-4.focus-visible\\:outline-offset-1.border.text-foreground.bg-transparent').first().click();
    const download = await downloadPromise;

    // ファイル名を生成（日付付き）
    const timestamp = new Date().toISOString().split('T')[0];
    const filename = `supabase-invoice-${timestamp}.pdf`;
    const filepath = path.join(DOWNLOAD_DIR, filename);

    await download.saveAs(filepath);
    log(`✓ ダウンロード完了: ${filepath}`);

  } catch (error) {
    logError('エラーが発生しました:', error);
    throw error;
  } finally {
    await browser.close();
  }
}

// 実行
downloadSupabaseInvoices()
  .then(() => {
    log('処理が完了しました');
    process.exit(0);
  })
  .catch((error) => {
    logError('処理に失敗しました:', error);
    process.exit(1);
  });

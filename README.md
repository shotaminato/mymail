# mymail

Windows / macOS /（将来）iOS 向けの軽量メールクライアントです。このリポジトリは **マイルストーン 1** までを実装しています。

- GUI: [Tauri 2](https://v2.tauri.app/) + Svelte 5 + TypeScript
- バックエンド: Rust（IMAP / MIME / SQLite）
- メールコア `crates/mail-core` は Tauri 非依存なので、ヘッドレスでテストできます

## 今できること

- 汎用 IMAP アカウントの追加（ホスト / ポート / TLS / ユーザー名 / パスワード）
- フォルダ一覧、フォルダ内メッセージ一覧（差出人・件名・日付・未読）
- 本文表示。HTML はサンドボックス iframe で描画し、スクリプト無効・リモート画像は既定でブロック
- SQLite にキャッシュし、FTS5 trigram トークナイザで件名 / 差出人 / 本文を検索（日本語の分かち書き不要）
- ローカルの GreenMail に対する Rust 統合テスト

まだ未実装（後続マイルストーン）:

- SMTP 送信、Gmail / Outlook の OAuth2、Yahoo アプリパスワードの専用設定
- OS キーチェーン（`keyring`）。現状のパスワード保存はアプリデータ内の JSON（暗号化なし）
- iOS ビルド、スレッド表示、添付ファイル、プッシュ同期

## 必要環境

- Rust 1.90+（`rustup`。Tauri 2.12 の要求に合わせています）
- Node.js 22 と [pnpm](https://pnpm.io/)
- [Tauri 2 の OS 前提条件](https://v2.tauri.app/start/prerequisites/)（Windows / macOS）
- テストメールサーバー用に **Docker**（推奨）または **Java 17+**

## テスト用メールサーバー（GreenMail）

実 Gmail アカウントなしで IMAP を試せます。ユーザーは自動では作らず、次の固定アカウントを使います。

| 項目 | 値 |
| --- | --- |
| IMAP | `127.0.0.1:3143`（TLS なし） |
| SMTP | `127.0.0.1:3025`（TLS なし） |
| IMAPS | `127.0.0.1:3993`（自己署名） |
| ユーザー名 | `testuser` |
| パスワード | `testpass` |
| メールアドレス | `testuser@localhost` |

### 1. サーバー起動

Docker がある場合:

```bash
docker compose up -d
```

Docker が無い場合（Java で同じスタンドアロン JAR を起動）:

```bash
chmod +x scripts/start-greenmail.sh
./scripts/start-greenmail.sh
```

起動確認: `127.0.0.1:3143` に TCP 接続できれば OK です。

### 2. サンプルメール投入

```bash
python3 scripts/seed_mail.py
```

次の 4 通が INBOX に入ります。

- 英語プレーンテキスト
- 日本語件名/本文（「会議の件」「全文検索テスト」）
- HTML（`<script>` とリモート画像付き。アプリでは無効化される想定）
- 日本語の短い連絡

GreenMail はメモリ上なので、サーバーを再起動したらもう一度 seed してください。

### 3. アプリから接続

```bash
pnpm install
pnpm tauri dev
```

1. 「アカウント追加」→「テストサーバー用に入力」（ホスト `127.0.0.1`、ポート `3143`、TLS なし、`testuser` / `testpass`）
2. 「接続して追加」
3. 左の INBOX を開き、メッセージと本文を確認
4. 上部の検索欄に `会議` や `全文検索` と入れて検索

## 開発コマンド

```bash
pnpm check          # フロントエンドの型チェック
pnpm build          # フロントエンドの本番ビルド
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm tauri build    # OS 向けパッケージ
```

統合テストは GreenMail が `127.0.0.1:3143` で動いている必要があります（CI では Docker サービスとして起動します）。ホストやポートを変える場合:

```bash
GREENMAIL_HOST=127.0.0.1 GREENMAIL_IMAP_PORT=3143 cargo test -p mail-core
```

## 構成

```
crates/mail-core/     IMAP・MIME・SQLite/FTS5・認証情報の抽象
src-tauri/            Tauri シェル（コマンドで mail-core を呼ぶ）
src/                  Svelte UI
scripts/              GreenMail 起動と seed
docker-compose.yml    GreenMail 公式イメージ greenmail/standalone
```

HTML 表示は空の `sandbox` 属性の iframe + 文書内 CSP（`default-src 'none'`、画像は既定で `data:` のみ）です。リモート画像はチェックボックスで明示的に許可できます。

認証情報は `CredentialStore` トレイト経由です。将来 `keyring` 実装に差し替えます。

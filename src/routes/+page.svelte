<script lang="ts">
  import { onMount } from "svelte";
  import {
    addAccount,
    getMessage,
    listAccounts,
    listFolders,
    removeAccount,
    searchMessages,
    syncFolder,
  } from "$lib/api";
  import { htmlToSrcdoc } from "$lib/sanitize";
  import type {
    Account,
    Folder,
    MessageBody,
    MessageSummary,
    NewAccount,
    TlsMode,
  } from "$lib/types";

  let accounts = $state<Account[]>([]);
  let folders = $state<Folder[]>([]);
  let messages = $state<MessageSummary[]>([]);
  let selectedAccountId = $state("");
  let selectedFolder = $state("");
  let selectedUid = $state<number | null>(null);
  let body = $state<MessageBody | null>(null);
  let query = $state("");
  let searching = $state(false);
  let busy = $state("");
  let error = $state("");
  let showForm = $state(false);
  let allowRemoteImages = $state(false);
  let showHtml = $state(true);

  let form: NewAccount & { password: string } = $state({
    displayName: "",
    imapHost: "127.0.0.1",
    imapPort: 3143,
    tlsMode: "none",
    username: "testuser",
    acceptInvalidCerts: true,
    password: "testpass",
  });

  const selectedAccount = $derived(
    accounts.find((a) => a.id === selectedAccountId) ?? null,
  );

  const srcdoc = $derived(
    body?.html && showHtml
      ? htmlToSrcdoc(body.html, allowRemoteImages)
      : null,
  );

  onMount(() => {
    void refreshAccounts();
  });

  async function run<T>(label: string, job: () => Promise<T>): Promise<T | undefined> {
    busy = label;
    error = "";
    try {
      return await job();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      return undefined;
    } finally {
      busy = "";
    }
  }

  async function refreshAccounts() {
    const list = await run("アカウント読み込み中", listAccounts);
    if (!list) return;
    accounts = list;
    if (!selectedAccountId && list[0]) {
      await selectAccount(list[0].id);
    }
  }

  async function selectAccount(id: string) {
    selectedAccountId = id;
    selectedFolder = "";
    selectedUid = null;
    body = null;
    messages = [];
    folders = [];
    query = "";
    searching = false;
    const list = await run("フォルダ取得中", () => listFolders(id));
    if (!list) return;
    folders = list;
    const inbox = list.find((f) => f.name.toUpperCase() === "INBOX") ?? list[0];
    if (inbox) await selectFolder(inbox.name, true);
  }

  async function selectFolder(name: string, forceSync = false) {
    if (!selectedAccountId) return;
    selectedFolder = name;
    selectedUid = null;
    body = null;
    searching = false;
    query = "";
    const list = await run("同期中", () =>
      forceSync || messages.length === 0
        ? syncFolder(selectedAccountId, name)
        : syncFolder(selectedAccountId, name),
    );
    if (list) messages = list;
  }

  async function openMessage(item: MessageSummary) {
    if (!selectedAccountId) return;
    selectedUid = item.uid;
    selectedFolder = item.folder;
    allowRemoteImages = false;
    showHtml = true;
    body = await run("本文取得中", () =>
      getMessage(selectedAccountId, item.folder, item.uid),
    ) ?? null;
  }

  async function onSearch() {
    if (!selectedAccountId) return;
    const q = query.trim();
    if (!q) {
      searching = false;
      if (selectedFolder) await selectFolder(selectedFolder, true);
      return;
    }
    searching = true;
    selectedUid = null;
    body = null;
    const hits = await run("検索中", () => searchMessages(selectedAccountId, q));
    if (hits) messages = hits;
  }

  function fillGreenmail() {
    form = {
      displayName: "GreenMail（ローカル）",
      imapHost: "127.0.0.1",
      imapPort: 3143,
      tlsMode: "none",
      username: "testuser",
      acceptInvalidCerts: true,
      password: "testpass",
    };
  }

  async function submitAccount(event: Event) {
    event.preventDefault();
    const created = await run("接続確認中", () =>
      addAccount(
        {
          displayName: form.displayName || form.username,
          imapHost: form.imapHost,
          imapPort: Number(form.imapPort),
          tlsMode: form.tlsMode as TlsMode,
          username: form.username,
          acceptInvalidCerts: form.acceptInvalidCerts,
        },
        form.password,
      ),
    );
    if (!created) return;
    showForm = false;
    accounts = [...accounts, created];
    await selectAccount(created.id);
  }

  async function dropAccount() {
    if (!selectedAccountId) return;
    if (!confirm("このアカウントを削除しますか？")) return;
    await run("削除中", () => removeAccount(selectedAccountId));
    selectedAccountId = "";
    await refreshAccounts();
  }

  function formatDate(value: string) {
    if (!value) return "";
    const d = new Date(value);
    if (Number.isNaN(d.getTime())) return value;
    return d.toLocaleString("ja-JP", { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
  }
</script>

<div class="app">
  <header class="top">
    <div class="brand">
      <strong>mymail</strong>
      <span>軽量メールクライアント</span>
    </div>
    <form class="search" onsubmit={(e) => { e.preventDefault(); void onSearch(); }}>
      <input
        placeholder="件名・差出人・本文を検索（日本語可）"
        bind:value={query}
        disabled={!selectedAccountId}
      />
      <button type="submit" disabled={!selectedAccountId}>検索</button>
    </form>
    <div class="top-actions">
      <button type="button" onclick={() => (showForm = !showForm)}>
        {showForm ? "閉じる" : "アカウント追加"}
      </button>
    </div>
  </header>

  {#if error}
    <div class="banner error">{error}</div>
  {/if}
  {#if busy}
    <div class="banner">{busy}…</div>
  {/if}

  {#if showForm}
    <form class="account-form" onsubmit={submitAccount}>
      <div class="form-head">
        <h2>IMAP アカウント</h2>
        <button type="button" class="link" onclick={fillGreenmail}>テストサーバー用に入力</button>
      </div>
      <label>表示名 <input bind:value={form.displayName} placeholder="GreenMail" /></label>
      <label>ホスト <input bind:value={form.imapHost} required /></label>
      <label>ポート <input type="number" bind:value={form.imapPort} required /></label>
      <label>TLS
        <select bind:value={form.tlsMode}>
          <option value="none">なし（GreenMail の 3143）</option>
          <option value="starttls">STARTTLS</option>
          <option value="implicit">暗黙 TLS（IMAPS）</option>
        </select>
      </label>
      <label>ユーザー名 <input bind:value={form.username} required /></label>
      <label>パスワード <input type="password" bind:value={form.password} required /></label>
      <label class="check">
        <input type="checkbox" bind:checked={form.acceptInvalidCerts} />
        自己署名証明書を許可する
      </label>
      <button type="submit">接続して追加</button>
    </form>
  {/if}

  <div class="panes">
    <aside class="sidebar">
      <div class="section-label">アカウント</div>
      {#if accounts.length === 0}
        <p class="hint">右上から IMAP アカウントを追加してください。ローカルの GreenMail なら「テストサーバー用に入力」が使えます。</p>
      {/if}
      {#each accounts as account}
        <button
          type="button"
          class="row"
          class:active={account.id === selectedAccountId}
          onclick={() => void selectAccount(account.id)}
        >
          <span class="title">{account.displayName}</span>
          <span class="meta">{account.username}</span>
        </button>
      {/each}
      {#if selectedAccount}
        <button type="button" class="link danger" onclick={() => void dropAccount()}>アカウント削除</button>
      {/if}

      <div class="section-label">フォルダ</div>
      {#each folders as folder}
        <button
          type="button"
          class="row"
          class:active={folder.name === selectedFolder && !searching}
          onclick={() => void selectFolder(folder.name, true)}
        >
          <span class="title">{folder.name}</span>
        </button>
      {/each}
    </aside>

    <section class="list">
      <div class="list-head">
        <strong>
          {#if searching}検索結果
          {:else if selectedFolder}{selectedFolder}
          {:else}メッセージ{/if}
        </strong>
        <span class="meta">{messages.length} 件</span>
        {#if selectedFolder && !searching}
          <button type="button" class="link" onclick={() => void selectFolder(selectedFolder, true)}>再同期</button>
        {/if}
      </div>
      {#if messages.length === 0}
        <p class="hint">メッセージはありません。</p>
      {/if}
      {#each messages as item}
        <button
          type="button"
          class="msg"
          class:active={item.uid === selectedUid && item.folder === selectedFolder}
          class:unseen={item.unseen}
          onclick={() => void openMessage(item)}
        >
          <div class="msg-top">
            <span class="from">{item.from || "(差出人なし)"}</span>
            <span class="date">{formatDate(item.date)}</span>
          </div>
          <div class="subject">{item.subject || "(件名なし)"}</div>
          {#if searching}
            <div class="meta">{item.folder}</div>
          {/if}
        </button>
      {/each}
    </section>

    <section class="reader">
      {#if !body}
        <div class="empty">メッセージを選ぶと本文が表示されます。</div>
      {:else}
        <div class="reader-head">
          <h1>{body.subject || "(件名なし)"}</h1>
          <div class="meta">{body.from}</div>
          <div class="meta">{body.date} · {body.folder} · UID {body.uid}</div>
          <div class="reader-actions">
            {#if body.html}
              <label class="check">
                <input type="checkbox" bind:checked={showHtml} />
                HTML で表示
              </label>
              <label class="check">
                <input type="checkbox" bind:checked={allowRemoteImages} disabled={!showHtml} />
                リモート画像を許可
              </label>
            {/if}
          </div>
          {#if body.html && showHtml}
            <p class="note">HTML はサンドボックス iframe で表示し、スクリプトは無効、リモート画像は既定でブロックします。</p>
          {/if}
        </div>
        {#if srcdoc}
          <iframe title="message-body" sandbox="" referrerpolicy="no-referrer" {srcdoc}></iframe>
        {:else}
          <pre class="plain">{body.text || "(本文なし)"}</pre>
        {/if}
      {/if}
    </section>
  </div>
</div>

<style>
  .app {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  .top {
    display: grid;
    grid-template-columns: 220px 1fr auto;
    gap: 16px;
    align-items: center;
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
  }
  .brand { display: flex; flex-direction: column; }
  .brand span, .meta, .hint, .note { color: var(--muted); }
  .search { display: flex; gap: 8px; }
  .search input { flex: 1; }
  input, select, button {
    border: 1px solid var(--line);
    background: #fff;
    border-radius: 8px;
    padding: 8px 10px;
  }
  button { cursor: pointer; background: var(--panel-2); }
  button:hover { border-color: var(--accent); }
  .banner {
    padding: 8px 16px;
    background: #ecfdf5;
    border-bottom: 1px solid var(--line);
  }
  .banner.error { background: #fef2f2; color: var(--danger); }
  .account-form {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px 16px;
    padding: 12px 16px;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
  }
  .form-head, .check, .account-form button[type="submit"] { grid-column: 1 / -1; }
  .form-head { display: flex; justify-content: space-between; align-items: center; }
  .form-head h2 { margin: 0; font-size: 16px; }
  label { display: flex; flex-direction: column; gap: 4px; }
  .check { flex-direction: row; align-items: center; gap: 8px; }
  .link { background: none; border: none; padding: 0; color: var(--accent-2); }
  .link.danger { color: var(--danger); margin: 8px 0 16px; }
  .panes { flex: 1; display: grid; grid-template-columns: 220px 340px 1fr; min-height: 0; }
  .sidebar, .list, .reader { min-height: 0; overflow: auto; }
  .sidebar { background: var(--panel-2); border-right: 1px solid var(--line); padding: 12px; }
  .section-label {
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted);
    margin: 12px 0 6px;
  }
  .row, .msg {
    display: block;
    width: 100%;
    text-align: left;
    margin-bottom: 6px;
    background: transparent;
  }
  .row.active, .msg.active { background: var(--selected); border-color: var(--accent); }
  .title { display: block; }
  .list { background: var(--panel); border-right: 1px solid var(--line); }
  .list-head, .reader-head { padding: 12px 14px; border-bottom: 1px solid var(--line); }
  .list-head { display: flex; gap: 10px; align-items: baseline; }
  .msg { border-radius: 0; border: none; border-bottom: 1px solid var(--line); padding: 10px 14px; }
  .msg-top { display: flex; justify-content: space-between; gap: 8px; }
  .msg.unseen .from, .msg.unseen .subject { font-weight: 700; }
  .from, .subject { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .reader { display: flex; flex-direction: column; background: #fff; }
  .reader h1 { margin: 0 0 6px; font-size: 20px; }
  .reader-actions { display: flex; gap: 16px; margin-top: 8px; }
  .empty, .hint { padding: 24px; }
  iframe {
    flex: 1;
    width: 100%;
    border: 0;
    background: #fff;
  }
  .plain {
    flex: 1;
    margin: 0;
    padding: 16px;
    white-space: pre-wrap;
    overflow: auto;
  }
  .note { font-size: 12px; margin: 8px 0 0; }
</style>

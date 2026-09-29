use mail_core::models::{Account, Folder, MessageBody, MessageSummary, NewAccount};
use mail_core::MailService;
use tauri::{Manager, State};

#[tauri::command]
fn list_accounts(service: State<'_, MailService>) -> Result<Vec<Account>, String> {
    service.list_accounts().map_err(|e| e.to_string())
}

#[tauri::command]
async fn add_account(
    service: State<'_, MailService>,
    account: NewAccount,
    password: String,
) -> Result<Account, String> {
    service
        .add_account(account, password)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_account(service: State<'_, MailService>, account_id: String) -> Result<(), String> {
    service
        .remove_account(&account_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_folders(
    service: State<'_, MailService>,
    account_id: String,
) -> Result<Vec<Folder>, String> {
    service
        .list_folders(&account_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn sync_folder(
    service: State<'_, MailService>,
    account_id: String,
    folder: String,
) -> Result<Vec<MessageSummary>, String> {
    service
        .sync_folder(&account_id, &folder)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_messages(
    service: State<'_, MailService>,
    account_id: String,
    folder: String,
) -> Result<Vec<MessageSummary>, String> {
    service
        .list_messages(&account_id, &folder)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_message(
    service: State<'_, MailService>,
    account_id: String,
    folder: String,
    uid: u32,
) -> Result<MessageBody, String> {
    service
        .get_message(&account_id, &folder, uid)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn search_messages(
    service: State<'_, MailService>,
    account_id: String,
    query: String,
) -> Result<Vec<MessageSummary>, String> {
    service
        .search(&account_id, &query)
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let service = MailService::open(dir.join("mail.db"), dir.join("credentials.json"))?;
            app.manage(service);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_accounts,
            add_account,
            remove_account,
            list_folders,
            sync_folder,
            list_messages,
            get_message,
            search_messages
        ])
        .run(tauri::generate_context!())
        .expect("error while running mymail");
}

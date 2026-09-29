import { invoke } from "@tauri-apps/api/core";
import type {
  Account,
  Folder,
  MessageBody,
  MessageSummary,
  NewAccount,
} from "./types";

export function listAccounts() {
  return invoke<Account[]>("list_accounts");
}

export function addAccount(account: NewAccount, password: string) {
  return invoke<Account>("add_account", { account, password });
}

export function removeAccount(accountId: string) {
  return invoke<void>("remove_account", { accountId });
}

export function listFolders(accountId: string) {
  return invoke<Folder[]>("list_folders", { accountId });
}

export function syncFolder(accountId: string, folder: string) {
  return invoke<MessageSummary[]>("sync_folder", { accountId, folder });
}

export function listMessages(accountId: string, folder: string) {
  return invoke<MessageSummary[]>("list_messages", { accountId, folder });
}

export function getMessage(accountId: string, folder: string, uid: number) {
  return invoke<MessageBody>("get_message", { accountId, folder, uid });
}

export function searchMessages(accountId: string, query: string) {
  return invoke<MessageSummary[]>("search_messages", { accountId, query });
}

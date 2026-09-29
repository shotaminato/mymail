export type TlsMode = "none" | "starttls" | "implicit";

export interface NewAccount {
  displayName: string;
  imapHost: string;
  imapPort: number;
  tlsMode: TlsMode;
  username: string;
  acceptInvalidCerts: boolean;
}

export interface Account extends NewAccount {
  id: string;
}

export interface Folder {
  name: string;
  delimiter: string | null;
}

export interface MessageSummary {
  uid: number;
  folder: string;
  from: string;
  subject: string;
  date: string;
  unseen: boolean;
}

export interface MessageBody extends MessageSummary {
  text: string | null;
  html: string | null;
}

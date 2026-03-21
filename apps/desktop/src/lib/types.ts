export enum LocalStorageKeys {
  ACEPTED_TERMS = "acceptedTerms",
}

export type CoreConnectionInfo = {
  ip: string | null;
  port: number;
  token: number;
  qr_payload: string | null;
};

export type CoreServiceStatus = {
  running: boolean;
  connection: CoreConnectionInfo | null;
};

export type WifiInfo = {
  connected: boolean;
  ssid: string | null;
  interface: string | null;
};

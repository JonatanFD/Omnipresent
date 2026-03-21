import { invoke } from "@tauri-apps/api/core";
import type { CoreServiceStatus, WifiInfo } from "@/lib/types";

export async function startCoreService(
  port: number,
  resetPin = false,
): Promise<CoreServiceStatus> {
  return invoke<CoreServiceStatus>("start_core_service", {
    port,
    resetPin,
  });
}

export async function stopCoreService(): Promise<CoreServiceStatus> {
  return invoke<CoreServiceStatus>("stop_core_service");
}

export async function getCoreStatus(): Promise<CoreServiceStatus> {
  return invoke<CoreServiceStatus>("get_core_status");
}

export async function getCurrentWifiInfo(): Promise<WifiInfo> {
  return invoke<WifiInfo>("get_current_wifi_info");
}

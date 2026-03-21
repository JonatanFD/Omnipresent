use serde::Serialize;
use std::process::Command;

#[derive(Clone, Debug, Serialize)]
pub struct WifiInfo {
    pub connected: bool,
    pub ssid: Option<String>,
    pub interface: Option<String>,
}

pub fn get_current_wifi() -> WifiInfo {
    #[cfg(target_os = "macos")]
    {
        return get_wifi_macos();
    }

    #[cfg(target_os = "linux")]
    {
        return get_wifi_linux();
    }

    #[cfg(target_os = "windows")]
    {
        return get_wifi_windows();
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        WifiInfo {
            connected: false,
            ssid: None,
            interface: None,
        }
    }
}

#[cfg(target_os = "macos")]
fn get_wifi_macos() -> WifiInfo {
    let output = Command::new(
        "/System/Library/PrivateFrameworks/Apple80211.framework/Versions/Current/Resources/airport",
    )
    .arg("-I")
    .output();

    if let Ok(result) = output {
        let raw = String::from_utf8_lossy(&result.stdout);
        let ssid = raw
            .lines()
            .find_map(|line| line.trim().strip_prefix("SSID: "))
            .map(ToOwned::to_owned);

        return WifiInfo {
            connected: ssid.is_some(),
            ssid,
            interface: None,
        };
    }

    WifiInfo {
        connected: false,
        ssid: None,
        interface: None,
    }
}

#[cfg(target_os = "linux")]
fn get_wifi_linux() -> WifiInfo {
    let output = Command::new("nmcli")
        .args(["-t", "-f", "active,ssid,device", "dev", "wifi"])
        .output();

    if let Ok(result) = output {
        let raw = String::from_utf8_lossy(&result.stdout);
        if let Some(active_line) = raw.lines().find(|line| line.starts_with("yes:")) {
            let mut parts = active_line.split(':');
            let _ = parts.next();
            let ssid = parts.next().map(ToOwned::to_owned);
            let interface = parts.next().map(ToOwned::to_owned);

            return WifiInfo {
                connected: ssid.as_deref().is_some_and(|value| !value.is_empty()),
                ssid,
                interface,
            };
        }
    }

    WifiInfo {
        connected: false,
        ssid: None,
        interface: None,
    }
}

#[cfg(target_os = "windows")]
fn get_wifi_windows() -> WifiInfo {
    let output = Command::new("netsh")
        .args(["wlan", "show", "interfaces"])
        .output();

    if let Ok(result) = output {
        let raw = String::from_utf8_lossy(&result.stdout);
        let mut ssid: Option<String> = None;
        let mut interface: Option<String> = None;

        for line in raw.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("Name") {
                if let Some(value) = trimmed.split(':').nth(1) {
                    interface = Some(value.trim().to_string());
                }
            }

            if trimmed.starts_with("SSID") && !trimmed.starts_with("SSID BSSID") {
                if let Some(value) = trimmed.split(':').nth(1) {
                    ssid = Some(value.trim().to_string());
                }
            }
        }

        return WifiInfo {
            connected: ssid.as_deref().is_some_and(|value| !value.is_empty()),
            ssid,
            interface,
        };
    }

    WifiInfo {
        connected: false,
        ssid: None,
        interface: None,
    }
}

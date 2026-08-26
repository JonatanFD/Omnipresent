//! What to tell the user so another machine can reach this one.

use std::net::{IpAddr, UdpSocket};

/// This machine's address on the local network, as a peer would dial it.
///
/// Found by asking the routing table rather than by listing interfaces: a UDP
/// socket that is *connected* sends nothing, but the kernel still has to pick a
/// source address, and reading it back gives the address on the interface that
/// actually carries traffic. Enumerating interfaces instead means choosing
/// between a loopback, a VPN, a bridge and half a dozen virtual adapters with
/// no way to tell which one the peer can see.
///
/// The address dialled is never contacted and does not need to exist.
#[tauri::command]
pub fn local_address() -> Option<String> {
    route_source("192.0.2.1:9")
        .or_else(|| route_source("[2001:db8::1]:9"))
        .map(|ip| ip.to_string())
}

/// The source address the kernel would use to reach `destination`.
///
/// `192.0.2.0/24` and `2001:db8::/32` are the documentation ranges — reserved
/// precisely so they can be named without belonging to anyone.
fn route_source(destination: &str) -> Option<IpAddr> {
    let bind = if destination.starts_with('[') {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    };
    let socket = UdpSocket::bind(bind).ok()?;
    socket.connect(destination).ok()?;
    let address = socket.local_addr().ok()?.ip();
    (!address.is_unspecified() && !address.is_loopback()).then_some(address)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_found_address_is_routable_not_loopback() {
        // On a machine with no network at all this is `None`, which is a valid
        // answer. What it must never be is 127.0.0.1 or 0.0.0.0 — a peer told to
        // dial either of those is being sent nowhere.
        if let Some(address) = local_address() {
            let parsed: IpAddr = address.parse().expect("a real address");
            assert!(!parsed.is_loopback(), "got loopback: {address}");
            assert!(!parsed.is_unspecified(), "got unspecified: {address}");
        }
    }

    #[test]
    fn an_unreachable_destination_still_yields_the_local_source() {
        // Nothing is sent, so the documentation address does not have to exist.
        // That is what makes this work offline and on an isolated LAN.
        if let Some(ip) = route_source("192.0.2.1:9") {
            assert!(ip.is_ipv4());
        }
    }
}

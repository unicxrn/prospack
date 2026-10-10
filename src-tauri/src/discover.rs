//! Finds FTP servers on the local network so users don't have to type their PS5's IP.
//! Scans each local IPv4 network (capped at a /24) on the usual PS5 FTP ports.

use serde::Serialize;
use std::collections::VecDeque;
use std::io::Read;
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// ftpsrv's default, then etaHEN's.
pub const PORTS: [u16; 2] = [2121, 1337];

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub ip: String,
    pub port: u16,
    pub banner: String,
    pub label: String,
}

impl Found {
    fn ps5_like(&self) -> bool {
        self.label != "FTP server"
    }
}

/// "ftpsrv v1.6.22-2-g49a54b-dirt", "etaHEN", or "FTP server".
pub fn label_for(banner: &str) -> String {
    if banner.contains("ftpsrv") {
        let version = banner
            .lines()
            .find_map(|l| l.split_once("Version:").map(|(_, v)| v.split_whitespace().next().unwrap_or("").to_string()))
            .filter(|v| !v.is_empty());
        return match version {
            Some(v) => format!("ftpsrv {v}"),
            None => "ftpsrv".into(),
        };
    }
    if banner.to_ascii_lowercase().contains("etahen") {
        return "etaHEN".into();
    }
    "FTP server".into()
}

/// Reads an FTP greeting until its final `220 ` line, 1 KB, or the timeout.
fn read_banner(mut s: TcpStream, timeout: Duration) -> Option<String> {
    let deadline = Instant::now() + timeout;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 256];
    while buf.len() < 1024 {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        s.set_read_timeout(Some(left)).ok()?;
        match s.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
        let text = String::from_utf8_lossy(&buf);
        if text.lines().any(|l| l.starts_with("220 ")) && text.ends_with('\n') {
            break;
        }
    }
    let text = String::from_utf8_lossy(&buf).into_owned();
    text.starts_with("220").then_some(text)
}

/// Probes every host:port pair with up to 256 parallel connects.
pub fn scan_hosts(hosts: &[Ipv4Addr], ports: &[u16], timeout: Duration) -> Vec<Found> {
    let queue: VecDeque<(Ipv4Addr, u16)> = hosts.iter().flat_map(|h| ports.iter().map(move |p| (*h, *p))).collect();
    let workers = queue.len().clamp(1, 256);
    let queue = Arc::new(Mutex::new(queue));
    let found = Arc::new(Mutex::new(Vec::new()));
    let handles: Vec<_> = (0..workers)
        .map(|_| {
            let (queue, found) = (queue.clone(), found.clone());
            std::thread::spawn(move || loop {
                let Some((ip, port)) = queue.lock().unwrap().pop_front() else { break };
                let addr = SocketAddr::from((ip, port));
                if let Ok(s) = TcpStream::connect_timeout(&addr, timeout) {
                    if let Some(banner) = read_banner(s, Duration::from_millis(600)) {
                        let label = label_for(&banner);
                        found.lock().unwrap().push(Found { ip: ip.to_string(), port, banner: banner.trim().to_string(), label });
                    }
                }
            })
        })
        .collect();
    for h in handles {
        let _ = h.join();
    }
    let mut out = Arc::try_unwrap(found).unwrap().into_inner().unwrap();
    out.sort_by(|a, b| {
        b.ps5_like()
            .cmp(&a.ps5_like())
            .then_with(|| a.ip.parse::<Ipv4Addr>().unwrap().cmp(&b.ip.parse::<Ipv4Addr>().unwrap()))
            .then(a.port.cmp(&b.port))
    });
    out
}

/// Container, VM and VPN adapters: a PS5 is never behind these.
pub fn is_virtual_adapter(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["docker", "br-", "virbr", "veth", "vmnet", "vboxnet", "tun", "tap", "wg", "tailscale", "zt", "lxc", "lxd", "podman", "cni", "flannel"]
        .iter()
        .any(|p| n.starts_with(p))
        || ["vethernet", "virtualbox", "vmware", "hyper-v", "wsl", "loopback", "bluetooth"].iter().any(|p| n.contains(p))
}

/// Hosts on this machine's real IPv4 networks (each capped at a /24), excluding itself.
pub fn local_hosts() -> Vec<Ipv4Addr> {
    let mut hosts = Vec::new();
    for iface in if_addrs::get_if_addrs().unwrap_or_default() {
        if is_virtual_adapter(&iface.name) {
            continue;
        }
        let if_addrs::IfAddr::V4(v4) = iface.addr else { continue };
        let ip = v4.ip;
        if ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() {
            continue;
        }
        let mask = u32::from(v4.netmask).max(0xFFFF_FF00); // never wider than a /24
        let net = u32::from(ip) & mask;
        let size = !mask;
        for host in 1..size {
            let candidate = Ipv4Addr::from(net | host);
            if candidate != ip && !hosts.contains(&candidate) {
                hosts.push(candidate);
            }
        }
    }
    hosts
}

pub fn discover() -> Vec<Found> {
    scan_hosts(&local_hosts(), &PORTS, Duration::from_millis(350))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fake_server(banner: &'static [u8]) -> u16 {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                let _ = s.write_all(banner);
            }
        });
        port
    }

    #[test]
    fn finds_ftpsrv_and_reads_its_version() {
        let port = fake_server(b"220-Welcome to ftpsrv.elf running on pid 135\r\n220-Version: v1.6.22-2-g49a54b-dirt\r\n220 Ready\r\n");
        // 127.0.0.2 has nothing listening on that port, so it must be skipped.
        let found = scan_hosts(&[Ipv4Addr::LOCALHOST, Ipv4Addr::new(127, 0, 0, 2)], &[port], Duration::from_millis(300));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].ip, "127.0.0.1");
        assert_eq!(found[0].label, "ftpsrv v1.6.22-2-g49a54b-dirt");
        let built = label_for("220-ftpsrv.elf\r\n220-Version: v1.6.22-2-g49a54b-dirty (built Sep 21 2026 02:27:03)\r\n220 ok\r\n");
        assert_eq!(built, "ftpsrv v1.6.22-2-g49a54b-dirty");
    }

    #[test]
    fn ignores_non_ftp_listeners() {
        let port = fake_server(b"SSH-2.0-OpenSSH_9.9\r\n");
        assert!(scan_hosts(&[Ipv4Addr::LOCALHOST], &[port], Duration::from_millis(300)).is_empty());
    }

    #[test]
    fn skips_virtual_adapters() {
        for n in ["docker0", "br-777c845ff02a", "virbr0", "veth12ab", "vEthernet (WSL)", "VirtualBox Host-Only Network", "tailscale0", "wg0"] {
            assert!(is_virtual_adapter(n), "{n}");
        }
        for n in ["enp5s0", "eth0", "wlan0", "Ethernet", "Wi-Fi", "Ethernet 2"] {
            assert!(!is_virtual_adapter(n), "{n}");
        }
    }

    #[test]
    fn labels() {
        assert_eq!(label_for("220 etaHEN FTP Server ready"), "etaHEN");
        assert_eq!(label_for("220 (vsFTPd 3.0.5)"), "FTP server");
        assert_eq!(label_for("220 ftpsrv"), "ftpsrv");
    }

    #[test]
    fn ps5_like_servers_sort_first() {
        let a = fake_server(b"220 (vsFTPd 3.0.5)\r\n");
        let b = fake_server(b"220-ftpsrv\r\n220-Version: v1\r\n220 ok\r\n");
        let found = scan_hosts(&[Ipv4Addr::LOCALHOST], &[a, b], Duration::from_millis(300));
        assert_eq!(found.iter().map(|f| f.label.as_str()).collect::<Vec<_>>(), vec!["ftpsrv v1", "FTP server"]);
    }

    /// Read-only scan of the real LAN: `cargo test discover_lan -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn discover_lan() {
        let t = Instant::now();
        let found = discover();
        eprintln!("{} hosts scanned in {:?}: {found:#?}", local_hosts().len(), t.elapsed());
    }
}

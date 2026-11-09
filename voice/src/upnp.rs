//! Opening the server's port on a home router automatically (UPnP), so friends outside the
//! house can join a server hosted from the app without anyone touching router settings.

use igd_next::aio::tokio::Tokio;
use igd_next::aio::Gateway;
use igd_next::{PortMappingProtocol, SearchOptions};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

const DESCRIPTION: &str = "Sompio Voice";

/// How the server can be reached from the internet.
pub enum Reach {
    /// The router opened the port: friends connect to `external:port`
    Open(Opened),
    /// No router answered (UPnP off, or not a home network)
    NoRouter,
    /// The router answered but wouldn't open the port
    Refused { external: Option<IpAddr>, reason: String },
    /// The internet provider shares one public address between customers (CGNAT); no port can be opened
    Shared { external: IpAddr },
}

pub struct Opened {
    pub external: IpAddr,
    pub port: u16,
    gateway: Gateway<Tokio>,
    renew: Option<tokio::task::JoinHandle<()>>,
}

impl Opened {
    /// Closes the port again (when the server stops).
    pub async fn close(mut self) {
        if let Some(r) = self.renew.take() {
            r.abort();
        }
        let _ = tokio::time::timeout(Duration::from_secs(3), self.gateway.remove_port(PortMappingProtocol::UDP, self.port)).await;
    }
}

/// Addresses that aren't reachable from the internet: private ranges, CGNAT (100.64/10), link-local.
pub fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            v.is_private() || v.is_loopback() || v.is_link_local() || v.is_unspecified() || (o[0] == 100 && (64..128).contains(&o[1]))
        }
        IpAddr::V6(v) => v.is_loopback() || v.is_unspecified() || (v.segments()[0] & 0xfe00) == 0xfc00 || (v.segments()[0] & 0xffc0) == 0xfe80,
    }
}

/// Asks the router to forward UDP `port` to this PC (`local_ip`).
pub async fn open(port: u16, local_ip: IpAddr) -> Reach {
    let IpAddr::V4(_) = local_ip else { return Reach::NoRouter };
    let opts = SearchOptions { timeout: Some(Duration::from_secs(3)), ..Default::default() };
    let Ok(gateway) = igd_next::aio::tokio::search_gateway(opts).await else { return Reach::NoRouter };
    let external = tokio::time::timeout(Duration::from_secs(3), gateway.get_external_ip()).await.ok().and_then(|r| r.ok());
    if let Some(ip) = external {
        if is_private(ip) {
            return Reach::Shared { external: ip };
        }
    }
    let local = SocketAddr::new(local_ip, port);
    // A lease of 0 means "until removed"; some routers only accept a time limit, then it's renewed.
    let first = tokio::time::timeout(Duration::from_secs(4), gateway.add_port(PortMappingProtocol::UDP, port, local, 0, DESCRIPTION)).await;
    let renew = match first {
        Ok(Ok(())) => None,
        _ => {
            let timed = tokio::time::timeout(Duration::from_secs(4), gateway.add_port(PortMappingProtocol::UDP, port, local, 3600, DESCRIPTION)).await;
            match timed {
                Ok(Ok(())) => {
                    let g = gateway.clone();
                    Some(tokio::spawn(async move {
                        loop {
                            tokio::time::sleep(Duration::from_secs(50 * 60)).await;
                            let _ = g.add_port(PortMappingProtocol::UDP, port, local, 3600, DESCRIPTION).await;
                        }
                    }))
                }
                Ok(Err(e)) => return Reach::Refused { external, reason: e.to_string() },
                Err(_) => return Reach::Refused { external, reason: "the router didn't answer".into() },
            }
        }
    };
    match external {
        Some(external) => Reach::Open(Opened { external, port, gateway, renew }),
        None => {
            let _ = gateway.remove_port(PortMappingProtocol::UDP, port).await;
            if let Some(r) = renew {
                r.abort();
            }
            Reach::Refused { external: None, reason: "the router didn't say what its public address is".into() }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_public_from_private() {
        for ip in ["192.168.1.1", "10.0.0.5", "172.20.1.1", "100.72.3.4", "169.254.1.1", "127.0.0.1", "fd00::1", "fe80::1"] {
            assert!(is_private(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["81.204.37.12", "100.128.0.1", "2a02:a46b::1"] {
            assert!(!is_private(ip.parse().unwrap()), "{ip}");
        }
    }

    #[tokio::test]
    async fn no_router_here_is_reported_not_hung() {
        let t = std::time::Instant::now();
        let r = open(9987, "192.168.1.50".parse().unwrap()).await;
        assert!(matches!(r, Reach::NoRouter | Reach::Refused { .. }));
        assert!(t.elapsed() < Duration::from_secs(15));
    }
}

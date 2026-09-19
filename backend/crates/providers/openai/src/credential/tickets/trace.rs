//! 不携带账号凭据；只有 trace 与打票共用同一 TCP 四元组时才确认出口 IP。
use hyper_util::client::legacy::connect::HttpInfo;
use reqwest::{Client, Response};
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

pub(super) struct ExitTrace {
    ip: String,
    connection: (SocketAddr, SocketAddr),
}

fn connection(response: &Response) -> Option<(SocketAddr, SocketAddr)> {
    response
        .extensions()
        .get::<HttpInfo>()
        .map(|info| (info.local_addr(), info.remote_addr()))
}

impl ExitTrace {
    pub(super) fn confirm(self, response: &Response) -> Option<String> {
        (connection(response) == Some(self.connection)).then_some(self.ip)
    }
}

pub(super) async fn observe(client: &Client, endpoint: &str) -> Option<ExitTrace> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut url = url::Url::parse(endpoint).ok()?;
        url.set_path("/cdn-cgi/trace");
        url.set_query(None);
        url.set_fragment(None);
        url.set_username("").ok()?;
        url.set_password(None).ok()?;
        let mut response = client
            .get(url)
            .header("accept", "text/plain")
            .header("cache-control", "no-cache")
            .send()
            .await
            .ok()?;
        if response.status() != 200 {
            return None;
        }
        let connection = connection(&response)?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            if bytes.len() + chunk.len() > 4096 {
                return None;
            }
            bytes.extend_from_slice(&chunk);
        }
        let body = std::str::from_utf8(&bytes).ok()?;
        let raw = body
            .lines()
            .find_map(|line| line.strip_prefix("ip="))?
            .trim();
        let ip = raw.parse::<IpAddr>().ok()?;
        let ip = match ip {
            IpAddr::V6(ip) => ip.to_ipv4_mapped().map_or(IpAddr::V6(ip), IpAddr::V4),
            ip => ip,
        };
        let public = match ip {
            IpAddr::V4(ip) => {
                !ip.is_private()
                    && !ip.is_loopback()
                    && !ip.is_link_local()
                    && !ip.is_unspecified()
                    && !ip.is_multicast()
                    && !ip.is_broadcast()
                    && !ip.is_documentation()
                    && ip.octets()[0] != 0
                    && ip.octets()[0] < 240
                    && !(ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1]))
                    && !(ip.octets()[0] == 198 && (18..=19).contains(&ip.octets()[1]))
            }
            IpAddr::V6(ip) => {
                (ip.segments()[0] & 0xe000) == 0x2000
                    && !(ip.segments()[0] == 0x2001 && ip.segments()[1] == 0xdb8)
            }
        };
        public.then(|| ExitTrace {
            ip: ip.to_string(),
            connection,
        })
    })
    .await
    .ok()
    .flatten()
}

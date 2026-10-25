//! Public Internet only: prevent external metadata/players reaching the host's LAN.
use reqwest::{
    dns::{Addrs, Name, Resolve, Resolving},
    Url,
};
use std::{net::IpAddr, sync::Arc};

pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            !(v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_broadcast()
                || v.is_documentation()
                || v.is_unspecified()
                || v.is_multicast()
                || o[0] == 0
                || o[0] >= 240
                || (o[0] == 100 && (64..=127).contains(&o[1]))
                || (o[0] == 198 && (18..=19).contains(&o[1])))
        }
        IpAddr::V6(v) => v.to_ipv4_mapped().map(|v| public_ip(v.into())).unwrap_or_else(|| {
            // Global unicast only; exclude documentation, transition and special ranges.
            let s = v.segments();
            (s[0] & 0xe000) == 0x2000 && s[0] != 0x2002 && !(s[0] == 0x2001 && (s[1] < 0x200 || s[1] == 0xdb8))
        }),
    }
}

pub fn valid_url(u: &Url) -> bool {
    if !matches!(u.scheme(), "http" | "https") || !u.username().is_empty() || u.password().is_some() {
        return false;
    }
    let Some(h) = u.host_str() else { return false };
    if let Ok(ip) = h.trim_matches(['[', ']']).parse::<IpAddr>() {
        return public_ip(ip);
    }
    h.contains('.') && !h.ends_with(".localhost") && !h.ends_with(".local")
}

pub struct PublicDns;
impl Resolve for PublicDns {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let addresses: Vec<_> = tokio::net::lookup_host((name.as_str(), 0)).await?.collect();
            if addresses.is_empty() || addresses.iter().any(|a| !public_ip(a.ip())) {
                return Err(std::io::Error::other("внутренний сетевой адрес запрещён").into());
            }
            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}

pub fn builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder().no_proxy().dns_resolver(Arc::new(PublicDns)).redirect(reqwest::redirect::Policy::custom(
        |attempt| {
            if attempt.previous().len() >= 6 || !valid_url(attempt.url()) {
                attempt.error("недопустимое перенаправление")
            } else {
                attempt.follow()
            }
        },
    ))
}

pub async fn limited(mut response: reqwest::Response, max: usize) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(response.content_length().is_none_or(|n| n <= max as u64), "ответ слишком большой");
    let mut data = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        anyhow::ensure!(data.len().saturating_add(chunk.len()) <= max, "ответ слишком большой");
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_internal_and_disguised_addresses() {
        for raw in [
            "http://127.0.0.1/",
            "http://2130706433/",
            "http://0x7f000001/",
            "http://10.0.0.1/",
            "http://192.168.1.1/",
            "http://169.254.169.254/",
            "http://100.64.0.1/",
            "http://[::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://[fc00::1]/",
            "http://localhost/",
            "file:///etc/passwd",
            "https://user:password@example.com/",
        ] {
            assert!(!valid_url(&Url::parse(raw).unwrap()), "{raw}");
        }
        assert!(valid_url(&Url::parse("https://video.example.com/").unwrap()));
        assert!(public_ip("8.8.8.8".parse().unwrap()));
    }
}

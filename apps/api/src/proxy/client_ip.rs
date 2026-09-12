use std::{
    net::IpAddr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue},
    middleware::Next,
    response::Response,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

const IP: &str = "x-anarlog-client-ip";
const TIME: &str = "x-anarlog-client-ip-time";
const SIGNATURE: &str = "x-anarlog-client-ip-signature";
const DOMAIN: &[u8] = b"anarlog.gateway.client-ip.v1\0";

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn mac(key: &str, ip: &str, time: &str) -> Hmac<Sha256> {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("HMAC accepts any key size");
    // The existing shared service key is never forwarded; separate this use from other HMACs.
    mac.update(DOMAIN);
    mac.update(ip.as_bytes());
    mac.update(b"\0");
    mac.update(time.as_bytes());
    mac
}

pub(super) fn sign(headers: &mut HeaderMap, key: &str) {
    for name in [IP, TIME, SIGNATURE] {
        headers.remove(name);
    }
    let Some(ip) = headers
        .get("fly-client-ip")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<IpAddr>().ok())
    else {
        return;
    };
    let ip = ip.to_string();
    let time = now().to_string();
    let signature = URL_SAFE_NO_PAD.encode(mac(key, &ip, &time).finalize().into_bytes());
    headers.insert(IP, HeaderValue::from_str(&ip).unwrap());
    headers.insert(TIME, HeaderValue::from_str(&time).unwrap());
    headers.insert(SIGNATURE, HeaderValue::from_str(&signature).unwrap());
}

fn verified(headers: &HeaderMap, key: &str, now: u64) -> Option<HeaderValue> {
    let ip = headers.get(IP)?.to_str().ok()?;
    ip.parse::<IpAddr>().ok()?;
    let time = headers.get(TIME)?.to_str().ok()?;
    if now.abs_diff(time.parse::<u64>().ok()?) > 60 {
        return None;
    }
    let signature = URL_SAFE_NO_PAD
        .decode(headers.get(SIGNATURE)?.as_bytes())
        .ok()?;
    mac(key, ip, time).verify_slice(&signature).ok()?;
    headers.get(IP).cloned()
}

pub async fn restore(State(key): State<Arc<str>>, mut request: Request, next: Next) -> Response {
    // A second public Fly Proxy replaces fly-client-ip with the gateway's IP. Only a
    // recent authenticated assertion may restore the original rate-limit identity.
    if let Some(ip) = verified(request.headers(), &key, now()) {
        request.headers_mut().insert("fly-client-ip", ip);
    }
    for name in [IP, TIME, SIGNATURE] {
        request.headers_mut().remove(name);
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsigned_tampered_expired_and_wrong_key_assertions_are_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert("fly-client-ip", HeaderValue::from_static("192.0.2.1"));
        headers.insert(IP, HeaderValue::from_static("192.0.2.99"));
        assert!(verified(&headers, "key", now()).is_none());
        sign(&mut headers, "key");
        assert_eq!(verified(&headers, "key", now()).unwrap(), "192.0.2.1");
        assert!(verified(&headers, "wrong-key", now()).is_none());
        let signed_at = headers[TIME].to_str().unwrap().parse::<u64>().unwrap();
        assert!(verified(&headers, "key", signed_at + 61).is_none());
        assert!(verified(&headers, "key", signed_at - 61).is_none());
        headers.insert(IP, HeaderValue::from_static("192.0.2.99"));
        assert!(verified(&headers, "key", now()).is_none());
        headers.remove("fly-client-ip");
        sign(&mut headers, "key");
        assert!(!headers.contains_key(SIGNATURE));
    }
}

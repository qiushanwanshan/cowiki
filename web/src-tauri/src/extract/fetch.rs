//! SSRF-aware HTTP fetch for static HTML. Redirects are followed manually so
//! every hop is checked; the response body is bounded before it is decoded.

use std::io::Read;
use std::net::IpAddr;
use std::time::Duration;

use url::Url;

use super::limits::{
    HTML_CONNECT_TIMEOUT_SECS, HTML_FETCH_TIMEOUT_SECS, MAX_HTML_REDIRECTS, MAX_HTML_RESPONSE_BYTES,
};

const USER_AGENT: &str = "Mozilla/5.0 (compatible; CoWiki/0.1; +https://github.com/wfnuser/cowiki)";

#[derive(Debug, Clone, Copy)]
pub struct FetchPolicy {
    allow_loopback: bool,
}

impl FetchPolicy {
    pub fn production() -> Self {
        Self {
            allow_loopback: false,
        }
    }

    #[cfg(test)]
    pub fn allow_loopback() -> Self {
        Self {
            allow_loopback: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FetchedPage {
    pub requested_url: String,
    pub final_url: String,
    pub status: u16,
    #[allow(dead_code)]
    pub content_type: String,
    pub html: String,
    pub response_bytes: u64,
}

pub fn fetch_url(url: &str) -> Result<FetchedPage, String> {
    fetch_url_with(url, FetchPolicy::production())
}

pub fn fetch_url_with(url: &str, policy: FetchPolicy) -> Result<FetchedPage, String> {
    let requested = parse_http_url(url)?;
    validate_fetch_target(&requested, policy)?;

    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(HTML_CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(HTML_FETCH_TIMEOUT_SECS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("cannot build HTTP client: {error}"))?;

    let mut current = requested.clone();
    for _ in 0..=MAX_HTML_REDIRECTS {
        validate_fetch_target(&current, policy)?;
        let response = client
            .get(current.clone())
            .send()
            .map_err(|error| map_fetch_error(&error))?;
        let status = response.status();
        if status.is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| "redirect is missing a Location header".to_string())?;
            let next = current
                .join(location)
                .map_err(|error| format!("redirect Location is not a valid URL: {error}"))?;
            if current.scheme() == "https" && next.scheme() != "https" {
                return Err("refusing to follow an HTTPS to HTTP redirect".to_string());
            }
            current = next;
            continue;
        }
        if !status.is_success() {
            return Err(http_status_error(status.as_u16(), &current));
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("text/html")
            .to_ascii_lowercase();
        if !is_html_content_type(&content_type) {
            return Err(format!(
                "response is not HTML (content-type: {content_type})"
            ));
        }
        if let Some(length) = response.content_length() {
            if length > MAX_HTML_RESPONSE_BYTES {
                return Err(format!(
                    "HTML response is too large ({length} bytes; maximum is {MAX_HTML_RESPONSE_BYTES})"
                ));
            }
        }
        let mut limited = response.take(MAX_HTML_RESPONSE_BYTES + 1);
        let mut bytes = Vec::new();
        limited
            .read_to_end(&mut bytes)
            .map_err(|error| format!("cannot read HTML response: {error}"))?;
        if bytes.len() as u64 > MAX_HTML_RESPONSE_BYTES {
            return Err(format!(
                "HTML response is too large (maximum is {MAX_HTML_RESPONSE_BYTES} bytes)"
            ));
        }
        let html = decode_html_bytes(&bytes, &content_type);
        return Ok(FetchedPage {
            requested_url: requested.as_str().to_string(),
            final_url: current.as_str().to_string(),
            status: status.as_u16(),
            content_type,
            response_bytes: bytes.len() as u64,
            html,
        });
    }
    Err(format!(
        "too many redirects (maximum is {MAX_HTML_REDIRECTS})"
    ))
}

pub fn parse_http_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value.trim()).map_err(|error| format!("invalid URL: {error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("only http and https URLs can be fetched".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("URLs with credentials are not allowed".to_string());
    }
    if url.host_str().is_none() {
        return Err("URL is missing a host".to_string());
    }
    Ok(url)
}

pub fn validate_fetch_target(url: &Url, policy: FetchPolicy) -> Result<(), String> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err("only http and https URLs can be fetched".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("URLs with credentials are not allowed".to_string());
    }
    let host = url
        .host_str()
        .ok_or_else(|| "URL is missing a host".to_string())?;
    if is_blocked_hostname(host) {
        return Err(format!("host '{host}' is not allowed"));
    }
    if !policy.allow_loopback {
        match url.port_or_known_default() {
            Some(80 | 443) => {}
            Some(port) => {
                return Err(format!(
                    "port {port} is not allowed; static fetch uses 80 or 443"
                ))
            }
            None => return Err("URL is missing a port".to_string()),
        }
    }
    let addrs = url
        .socket_addrs(|| url.port_or_known_default())
        .map_err(|error| format!("cannot resolve host '{host}': {error}"))?;
    if addrs.is_empty() {
        return Err(format!("host '{host}' did not resolve to any address"));
    }
    for addr in addrs {
        if ip_is_blocked(addr.ip(), policy) {
            return Err(format!(
                "host '{host}' resolves to a private or reserved address"
            ));
        }
    }
    Ok(())
}

fn is_html_content_type(content_type: &str) -> bool {
    let mime = content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim();
    matches!(
        mime,
        "text/html" | "application/xhtml+xml" | "application/xml" | "text/xml" | "text/plain"
    )
}

fn decode_html_bytes(bytes: &[u8], content_type: &str) -> String {
    let label = content_type
        .split(';')
        .filter_map(|part| {
            let part = part.trim();
            part.split_once('=')
                .filter(|(key, _)| key.eq_ignore_ascii_case("charset"))
                .map(|(_, value)| value.trim_matches(|ch| ch == '"' || ch == '\'').trim())
        })
        .next()
        .unwrap_or("utf-8");
    let encoding = encoding_rs::Encoding::for_label(label.as_bytes()).unwrap_or(encoding_rs::UTF_8);
    encoding.decode(bytes).0.into_owned()
}

fn http_status_error(status: u16, url: &Url) -> String {
    match status {
        401 | 403 => {
            format!("this page looks like a login or challenge wall (HTTP {status} from {url})")
        }
        404 => format!("page not found (HTTP 404 from {url})"),
        429 => format!("the site rate-limited the request (HTTP 429 from {url})"),
        other => format!("HTTP {other} fetching {url}"),
    }
}

fn map_fetch_error(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "timed out fetching the page".to_string()
    } else if error.is_connect() {
        format!("cannot connect to the page: {error}")
    } else {
        format!("cannot fetch the page: {error}")
    }
}

fn is_blocked_hostname(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".internal")
        || host.ends_with(".local")
        || host == "metadata.google.internal"
}

fn ip_is_blocked(ip: IpAddr, policy: FetchPolicy) -> bool {
    if policy.allow_loopback && ip.is_loopback() {
        return false;
    }
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.is_multicast()
                || octets[0] == 0
                || (octets[0] == 100 && (64..=127).contains(&octets[1]))
                || octets[0] >= 240
        }
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return ip_is_blocked(IpAddr::V4(mapped), policy);
            }
            let segments = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (segments[0] & 0xfe00) == 0xfc00
                || (segments[0] & 0xffc0) == 0xfe80
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn rejects_non_http_schemes_and_credentials() {
        let policy = FetchPolicy::production();
        assert!(parse_http_url("file:///etc/passwd")
            .unwrap_err()
            .contains("http"));
        assert!(parse_http_url("javascript:alert(1)")
            .unwrap_err()
            .contains("http"));
        assert!(parse_http_url("https://user:secret@example.com/")
            .unwrap_err()
            .contains("credentials"));
        let local = Url::parse("http://127.0.0.1/").unwrap();
        assert!(validate_fetch_target(&local, policy)
            .unwrap_err()
            .contains("private or reserved"));
        let metadata = Url::parse("http://169.254.169.254/latest").unwrap();
        assert!(validate_fetch_target(&metadata, policy)
            .unwrap_err()
            .contains("private or reserved"));
        let odd_port = Url::parse("https://example.com:22/").unwrap();
        assert!(validate_fetch_target(&odd_port, policy)
            .unwrap_err()
            .contains("port 22"));
    }

    #[test]
    fn loopback_policy_can_fetch_a_local_html_fixture() {
        let url = serve_html(
            200,
            "text/html; charset=utf-8",
            "<html><body><p>Local fixture article.</p></body></html>",
        );
        let page = fetch_url_with(&url, FetchPolicy::allow_loopback()).unwrap();
        assert_eq!(page.status, 200);
        assert!(page.html.contains("Local fixture article"));
        assert_eq!(page.requested_url, page.final_url);
    }

    #[test]
    fn follows_a_relative_redirect_on_loopback() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            for turn in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let response = if turn == 0 {
                    "HTTP/1.1 302 Found\r\nLocation: /article\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        .to_string()
                } else {
                    let body = "<html><body><p>Redirected article body with enough text.</p></body></html>";
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                };
                let _ = stream.write_all(response.as_bytes());
            }
        });
        let start = format!("http://127.0.0.1:{}/start", addr.port());
        let page = fetch_url_with(&start, FetchPolicy::allow_loopback()).unwrap();
        assert!(page.final_url.ends_with("/article"));
        assert!(page.html.contains("Redirected article"));
    }

    #[test]
    fn rejects_non_html_content_type() {
        let url = serve_html(200, "application/pdf", "%PDF-1.4");
        let error = fetch_url_with(&url, FetchPolicy::allow_loopback()).unwrap_err();
        assert!(error.contains("not HTML"));
    }

    fn serve_html(status: u16, content_type: &str, body: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let content_type = content_type.to_string();
        let body = body.to_string();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 {status} OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        });
        format!("http://127.0.0.1:{}", addr.port())
    }
}

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use mihomo_tray_service::core_ipc_path;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::ProfileMeta;
use crate::paths::{DELAY_TEST_URL, DELAY_TIMEOUT_MS};

#[derive(Debug, Clone)]
pub struct ApiClient {
    socket: PathBuf,
    secret: Option<String>,
    timeout: Duration,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyInfo {
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub now: Option<String>,
    pub all: Option<Vec<String>>,
    pub history: Option<Vec<DelayHistory>>,
    #[allow(dead_code)]
    #[serde(default)]
    pub alive: Option<bool>,
    pub hidden: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DelayHistory {
    pub delay: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxiesResponse {
    pub proxies: HashMap<String, ProxyInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionInfo {
    #[allow(dead_code)]
    pub version: Option<String>,
    #[allow(dead_code)]
    pub meta: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeConfig {
    #[serde(rename = "mixed-port")]
    pub mixed_port: Option<u16>,
    pub port: Option<u16>,
    #[serde(rename = "socks-port")]
    pub socks_port: Option<u16>,
    pub tun: Option<TunConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TunConfig {
    pub enable: Option<bool>,
    pub stack: Option<String>,
    #[serde(rename = "auto-route")]
    pub auto_route: Option<bool>,
    #[serde(rename = "auto-detect-interface")]
    pub auto_detect_interface: Option<bool>,
    #[serde(rename = "dns-hijack")]
    pub dns_hijack: Option<Vec<String>>,
}

const GROUP_TYPES: &[&str] = &["Selector", "URLTest", "Fallback", "LoadBalance", "Relay"];

impl ApiClient {
    pub fn from_profile(meta: &ProfileMeta) -> Self {
        Self {
            socket: core_ipc_path(),
            secret: meta.secret().map(|s| s.to_string()),
            timeout: Duration::from_secs(15),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn request(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Vec<u8>> {
        let body_str = match body {
            Some(b) => serde_json::to_string(b).context("serialize request body")?,
            None => String::new(),
        };
        let auth = self
            .secret
            .as_ref()
            .map(|s| format!("Authorization: Bearer {s}\r\n"))
            .unwrap_or_default();
        let request = format!(
            "{method} {path} HTTP/1.1\r\n\
             Host: localhost\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             {auth}\r\n\
             {body_str}",
            body_str.len(),
        );

        let mut stream = connect_local(&self.socket, self.timeout)
            .with_context(|| format!("connect {}", self.socket.display()))?;
        stream
            .write_all(request.as_bytes())
            .context("write API request")?;
        stream.flush().context("flush API request")?;

        let (status, payload) = read_http_response(&mut stream).context("read API response")?;
        if !(200..300).contains(&status) {
            let msg = String::from_utf8_lossy(&payload);
            bail!("{method} {path} -> HTTP {status}: {msg}");
        }
        Ok(payload)
    }

    pub fn version(&self) -> Result<VersionInfo> {
        let body = self.request("GET", "/version", None).context("GET /version")?;
        Ok(serde_json::from_slice(&body)?)
    }

    pub fn wait_ready(&self, attempts: u32, delay_ms: u64) -> Result<()> {
        let mut last = None;
        for _ in 0..attempts {
            match self.version() {
                Ok(_) => return Ok(()),
                Err(e) => {
                    last = Some(e);
                    std::thread::sleep(Duration::from_millis(delay_ms));
                }
            }
        }
        bail!("mihomo API not ready: {:?}", last)
    }

    pub fn proxies(&self) -> Result<ProxiesResponse> {
        let body = self.request("GET", "/proxies", None).context("GET /proxies")?;
        let mut parsed: ProxiesResponse = serde_json::from_slice(&body)?;
        for (key, value) in parsed.proxies.iter_mut() {
            if value.name.is_empty() {
                value.name = key.clone();
            }
        }
        Ok(parsed)
    }

    pub fn proxy_groups(&self) -> Result<Vec<ProxyInfo>> {
        let resp = self.proxies()?;
        let mut groups: Vec<_> = resp
            .proxies
            .into_iter()
            .map(|(key, mut p)| {
                if p.name.is_empty() {
                    p.name = key;
                }
                p
            })
            .filter(|p| GROUP_TYPES.iter().any(|t| t.eq_ignore_ascii_case(&p.proxy_type)))
            .filter(|p| !p.hidden.unwrap_or(false))
            .filter(|p| p.name != "GLOBAL")
            .collect();
        groups.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(groups)
    }

    pub fn select_proxy(&self, group: &str, name: &str) -> Result<()> {
        let path = format!("/proxies/{}", urlencoding(group));
        let body = serde_json::json!({ "name": name });
        self.request("PUT", &path, Some(&body))
            .context("PUT /proxies")?;
        Ok(())
    }

    pub fn group_delay(&self, group: &str) -> Result<HashMap<String, u16>> {
        let path = format!(
            "/group/{}/delay?url={}&timeout={}",
            urlencoding(group),
            urlencoding(DELAY_TEST_URL),
            DELAY_TIMEOUT_MS
        );
        let body = self
            .clone()
            .with_timeout(Duration::from_secs(60))
            .request("GET", &path, None)
            .context("GET /group/delay")?;
        Ok(serde_json::from_slice(&body)?)
    }

    pub fn configs(&self) -> Result<RuntimeConfig> {
        let body = self.request("GET", "/configs", None).context("GET /configs")?;
        Ok(serde_json::from_slice(&body)?)
    }

    pub fn patch_configs(&self, body: Value) -> Result<()> {
        self.request("PATCH", "/configs", Some(&body))
            .context("PATCH /configs")?;
        Ok(())
    }

    pub fn reload_config(&self, path: &str) -> Result<()> {
        let body = serde_json::json!({ "path": path, "payload": "" });
        self.request("PUT", "/configs?force=true", Some(&body))
            .context("PUT /configs")?;
        Ok(())
    }

    pub fn set_tun_enabled(&self, enable: bool) -> Result<()> {
        let body = if enable {
            serde_json::json!({
                "tun": {
                    "enable": true,
                    "stack": "mixed",
                    "auto-route": true,
                    "auto-detect-interface": true,
                    "dns-hijack": ["any:53"]
                }
            })
        } else {
            serde_json::json!({
                "tun": { "enable": false }
            })
        };
        self.patch_configs(body)
    }

    pub fn http_port(&self) -> Result<u16> {
        let cfg = self.configs()?;
        // Mihomo returns 0 for unset listeners; treat 0 as absent.
        Ok(nonzero_port(cfg.mixed_port)
            .or_else(|| nonzero_port(cfg.port))
            .unwrap_or(7890))
    }

    pub fn socks_port(&self) -> Result<u16> {
        let cfg = self.configs()?;
        Ok(nonzero_port(cfg.socks_port)
            .or_else(|| nonzero_port(cfg.mixed_port))
            .unwrap_or(7890))
    }

    pub fn tun_enabled(&self) -> Result<bool> {
        let cfg = self.configs()?;
        Ok(cfg.tun.and_then(|t| t.enable).unwrap_or(false))
    }
}

fn nonzero_port(port: Option<u16>) -> Option<u16> {
    port.filter(|&p| p != 0)
}

fn connect_local(path: &PathBuf, timeout: Duration) -> Result<LocalStream> {
    #[cfg(unix)]
    {
        use std::os::unix::net::UnixStream;
        let stream = UnixStream::connect(path)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        Ok(LocalStream::Unix(stream))
    }
    #[cfg(windows)]
    {
        use std::fs::OpenOptions;

        let pipe = path.to_string_lossy();
        let pipe_name = if pipe.starts_with(r"\\.\pipe\") {
            pipe.into_owned()
        } else {
            format!(r"\\.\pipe\{}", pipe.trim_start_matches('/'))
        };

        // Retry briefly while the core creates the pipe.
        let mut last = None;
        for _ in 0..40 {
            match OpenOptions::new().read(true).write(true).open(&pipe_name) {
                Ok(file) => {
                    let _ = timeout;
                    return Ok(LocalStream::Pipe(file));
                }
                Err(e) => {
                    last = Some(e);
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
        Err(last
            .map(Into::into)
            .unwrap_or_else(|| anyhow::anyhow!("failed to open named pipe {pipe_name}")))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, timeout);
        bail!("local mihomo IPC is not supported on this platform")
    }
}

enum LocalStream {
    #[cfg(unix)]
    Unix(std::os::unix::net::UnixStream),
    #[cfg(windows)]
    Pipe(std::fs::File),
}

impl Read for LocalStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            #[cfg(unix)]
            Self::Unix(s) => s.read(buf),
            #[cfg(windows)]
            Self::Pipe(s) => s.read(buf),
        }
    }
}

impl Write for LocalStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            #[cfg(unix)]
            Self::Unix(s) => s.write(buf),
            #[cfg(windows)]
            Self::Pipe(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Unix(s) => s.flush(),
            #[cfg(windows)]
            Self::Pipe(s) => s.flush(),
        }
    }
}

fn read_http_response(stream: &mut impl Read) -> Result<(u16, Vec<u8>)> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let header_end = loop {
        let n = stream.read(&mut tmp).context("read response")?;
        if n == 0 {
            bail!("unexpected EOF while reading HTTP headers");
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = find_header_end(&buf) {
            break pos;
        }
        if buf.len() > 64 * 1024 {
            bail!("HTTP headers too large");
        }
    };

    let (header_bytes, rest) = buf.split_at(header_end);
    let headers = std::str::from_utf8(header_bytes).context("HTTP headers are not UTF-8")?;
    let mut lines = headers.split("\r\n");
    let status_line = lines.next().context("missing status line")?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .context("missing status code")?
        .parse::<u16>()
        .context("invalid status code")?;

    let mut content_length = None;
    let mut chunked = false;
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            content_length = Some(v.trim().parse::<usize>().context("content-length")?);
        } else if lower.starts_with("transfer-encoding:") && lower.contains("chunked") {
            chunked = true;
        }
    }

    let mut body = rest.to_vec();
    if let Some(len) = content_length {
        while body.len() < len {
            let n = stream.read(&mut tmp).context("read body")?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&tmp[..n]);
        }
        body.truncate(len);
    } else if chunked {
        body = decode_chunked(&mut body, stream)?;
    } else {
        // No length: read until EOF (common for some local servers).
        loop {
            let n = stream.read(&mut tmp).context("read body until eof")?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&tmp[..n]);
        }
    }

    Ok((status, body))
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

fn decode_chunked(initial: &mut Vec<u8>, stream: &mut impl Read) -> Result<Vec<u8>> {
    let mut data = std::mem::take(initial);
    let mut out = Vec::new();
    let mut tmp = [0u8; 4096];

    loop {
        while !data.windows(2).any(|w| w == b"\r\n") {
            let n = stream.read(&mut tmp).context("read chunk size")?;
            if n == 0 {
                bail!("unexpected EOF in chunked body");
            }
            data.extend_from_slice(&tmp[..n]);
        }
        let line_end = data.windows(2).position(|w| w == b"\r\n").unwrap();
        let size_line = std::str::from_utf8(&data[..line_end])
            .context("chunk size is not UTF-8")?
            .trim();
        let size = usize::from_str_radix(size_line.split(';').next().unwrap_or(""), 16)
            .context("invalid chunk size")?;
        data.drain(..line_end + 2);
        if size == 0 {
            break;
        }
        while data.len() < size + 2 {
            let n = stream.read(&mut tmp).context("read chunk data")?;
            if n == 0 {
                bail!("unexpected EOF in chunk data");
            }
            data.extend_from_slice(&tmp[..n]);
        }
        out.extend_from_slice(&data[..size]);
        data.drain(..size + 2);
    }
    Ok(out)
}

fn urlencoding(s: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0xf) as usize] as char);
            }
        }
    }
    out
}

pub fn latest_delay(info: &ProxyInfo) -> Option<u16> {
    info.history.as_ref()?.last().map(|h| h.delay)
}

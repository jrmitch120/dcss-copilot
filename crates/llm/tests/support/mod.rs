//! A fake LLM provider endpoint: a localhost HTTP server that answers each request with a canned
//! response and remembers what it was sent. No test ever reaches a live model.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// One request the fake endpoint received.
#[derive(Debug, Clone)]
pub struct Received {
    pub method: String,
    pub path: String,
    /// Header names in lower case.
    pub headers: BTreeMap<String, String>,
    pub body: serde_json::Value,
}

/// What the fake endpoint answers every request with.
#[derive(Debug, Clone)]
pub struct Canned {
    pub status: u16,
    pub content_type: &'static str,
    pub body: String,
}

impl Canned {
    /// A `200` streaming Server-Sent Events body.
    pub fn events(body: &str) -> Self {
        Self {
            status: 200,
            content_type: "text/event-stream",
            body: body.to_owned(),
        }
    }

    /// An error status with a JSON body.
    pub fn error(status: u16, body: &str) -> Self {
        Self {
            status,
            content_type: "application/json",
            body: body.to_owned(),
        }
    }
}

/// A running fake endpoint.
pub struct FakeEndpoint {
    pub url: String,
    received: Arc<Mutex<Vec<Received>>>,
}

impl FakeEndpoint {
    /// Starts answering every request with `canned`.
    pub async fn start(canned: Canned) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let received = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&received);
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let canned = canned.clone();
                let log = Arc::clone(&log);
                tokio::spawn(async move { answer(socket, &canned, &log).await });
            }
        });
        Self {
            url: format!("http://{addr}"),
            received,
        }
    }

    /// Every request received so far.
    pub fn received(&self) -> Vec<Received> {
        self.received.lock().unwrap().clone()
    }
}

async fn answer(mut socket: TcpStream, canned: &Canned, log: &Mutex<Vec<Received>>) {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let n = socket.read(&mut chunk).await.unwrap();
        assert!(n > 0, "connection closed before the request headers ended");
        bytes.extend_from_slice(&chunk[..n]);
        if let Some(at) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break at + 4;
        }
    };
    let head = String::from_utf8(bytes[..head_end].to_vec()).unwrap();
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next().unwrap().split(' ');
    let method = request_line.next().unwrap().to_owned();
    let path = request_line.next().unwrap().to_owned();
    let headers: BTreeMap<String, String> = lines
        .filter_map(|line| line.split_once(": "))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.to_owned()))
        .collect();
    let length: usize = headers
        .get("content-length")
        .map_or(0, |value| value.parse().unwrap());
    while bytes.len() < head_end + length {
        let n = socket.read(&mut chunk).await.unwrap();
        bytes.extend_from_slice(&chunk[..n]);
    }
    let body = &bytes[head_end..head_end + length];
    let body = if body.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(body).unwrap()
    };
    log.lock().unwrap().push(Received {
        method,
        path,
        headers,
        body,
    });

    let response = format!(
        "HTTP/1.1 {} Canned\r\ncontent-type: {}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        canned.status,
        canned.content_type,
        canned.body.len(),
        canned.body
    );
    socket.write_all(response.as_bytes()).await.unwrap();
    socket.shutdown().await.unwrap();
}

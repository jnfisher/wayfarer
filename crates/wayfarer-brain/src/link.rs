//! The brain's connection to the gateway: a [`Link`] over a WebSocket ([`WsLink`]) or over a
//! pair of channels ([`ChannelLink`]: tests, and the in-process brain of `wayfarer serve`).
//!
//! Sending never waits: each link queues what the brain sends and writes it from a task of its
//! own, so the brain is always free to read. The gateway's connection to the brain waits for the
//! brain to read before it reads the brain's own messages again; a brain that waited to send
//! could stall both.

use std::future::Future;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use wayfarer_proto::{ToClient, ToGateway};

/// Messages read from the socket before the reader waits for the brain.
const INBOX: usize = 1024;

/// One connection to the gateway.
pub trait Link: Send {
    /// Queue a message for the gateway. Never waits; a message sent after the connection
    /// ended is dropped (`recv` then says so).
    fn send(&mut self, msg: ToGateway);
    /// The next message from the gateway; `None` once the connection has ended. Cancel safe.
    fn recv(&mut self) -> impl Future<Output = Option<ToClient>> + Send;
}

/// A link over a pair of channels, as `Hub::connect` gives them.
pub struct ChannelLink {
    out: mpsc::UnboundedSender<ToGateway>,
    rx: mpsc::Receiver<ToClient>,
    /// Ends once the link is dropped and what it queued has gone.
    _writer: JoinHandle<()>,
}

impl ChannelLink {
    /// Must be called inside a tokio runtime.
    pub fn new(tx: mpsc::Sender<ToGateway>, rx: mpsc::Receiver<ToClient>) -> Self {
        let (out, mut queue) = mpsc::unbounded_channel();
        let writer = tokio::spawn(async move {
            while let Some(msg) = queue.recv().await {
                if tx.send(msg).await.is_err() {
                    break;
                }
            }
        });
        Self {
            out,
            rx,
            _writer: writer,
        }
    }
}

impl Link for ChannelLink {
    fn send(&mut self, msg: ToGateway) {
        let _ = self.out.send(msg);
    }

    async fn recv(&mut self) -> Option<ToClient> {
        self.rx.recv().await
    }
}

#[derive(Debug, thiserror::Error)]
#[error("cannot reach the gateway at {url}: {source}")]
pub struct ConnectError {
    pub url: String,
    #[source]
    pub source: tokio_tungstenite::tungstenite::Error,
}

/// A link over the gateway's WebSocket (`ws://HOST:PORT/ws`). Dropping it closes the socket.
pub struct WsLink {
    out: Option<mpsc::UnboundedSender<ToGateway>>,
    rx: mpsc::Receiver<ToClient>,
    reader: JoinHandle<()>,
    writer: Option<JoinHandle<()>>,
}

impl WsLink {
    pub async fn connect(url: &str) -> Result<Self, ConnectError> {
        let (ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .map_err(|source| ConnectError {
                url: url.to_owned(),
                source,
            })?;
        let (mut sink, mut stream) = ws.split();
        let (out, mut queue) = mpsc::unbounded_channel::<ToGateway>();
        let writer = tokio::spawn(async move {
            while let Some(msg) = queue.recv().await {
                if sink.send(Message::text(msg.to_json())).await.is_err() {
                    return;
                }
            }
            let _ = sink.close().await;
        });
        let (tx, rx) = mpsc::channel(INBOX);
        let reader = tokio::spawn(async move {
            while let Some(frame) = stream.next().await {
                let text = match frame {
                    Ok(Message::Text(text)) => text,
                    Ok(Message::Close(_)) | Err(_) => break,
                    Ok(_) => continue,
                };
                match ToClient::from_json(text.as_str()) {
                    Ok(msg) => {
                        if tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                    // A newer gateway's message: report it, as the protocol says.
                    Err(e) => tracing::warn!("from the gateway: {e}"),
                }
            }
        });
        Ok(Self {
            out: Some(out),
            rx,
            reader,
            writer: Some(writer),
        })
    }

    /// Send what is queued, then close the socket (waits at most a second).
    pub async fn close(mut self) {
        self.out.take();
        if let Some(writer) = self.writer.take() {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(1), writer).await;
        }
    }
}

impl Drop for WsLink {
    fn drop(&mut self) {
        self.reader.abort();
        if let Some(w) = &self.writer {
            w.abort();
        }
    }
}

impl Link for WsLink {
    fn send(&mut self, msg: ToGateway) {
        if let Some(out) = &self.out {
            let _ = out.send(msg);
        }
    }

    async fn recv(&mut self) -> Option<ToClient> {
        self.rx.recv().await
    }
}

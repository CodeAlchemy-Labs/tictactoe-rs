//! WebSocket transport implementation.
//!
//! The transport relies on `tokio-tungstenite` for the wire protocol. TLS is
//! delegated to `rustls` with the system CA bundle. When `insecure` is set,
//! the transport installs a connector that accepts any certificate; this is
//! intended for development servers with self-signed certificates and must
//! not be used against production endpoints.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async_tls_with_config, Connector};

use common::protocol::{ClientMessage, ServerMessage};

use crate::infrastructure::transport::{Transport, TransportHandle};

/// A WebSocket transport that connects to a `ws://` or `wss://` URL.
pub struct WsTransport {
    url: String,
    insecure: bool,
}

impl WsTransport {
    /// Creates a transport that will connect to `url` when started.
    ///
    /// When `insecure` is `true`, the transport disables TLS certificate
    /// verification.
    pub fn new(url: impl Into<String>, insecure: bool) -> Self {
        Self {
            url: url.into(),
            insecure,
        }
    }
}

impl Transport for WsTransport {
    fn start(self) -> TransportHandle {
        let (outgoing_tx, outgoing_rx) = mpsc::unbounded_channel::<ClientMessage>();
        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel::<ServerMessage>();

        let url = self.url;
        let insecure = self.insecure;
        tokio::spawn(run_connection(url, insecure, outgoing_rx, incoming_tx));

        TransportHandle {
            outgoing: outgoing_tx,
            incoming: incoming_rx,
        }
    }
}

/// Drives a WebSocket connection until it is closed.
async fn run_connection(
    url: String,
    insecure: bool,
    mut outgoing_rx: mpsc::UnboundedReceiver<ClientMessage>,
    incoming_tx: mpsc::UnboundedSender<ServerMessage>,
) {
    let connector = insecure.then(insecure_connector);
    let connect_result = connect_async_tls_with_config(&url, None, false, connector).await;

    let (socket, _response) = match connect_result {
        Ok(pair) => pair,
        Err(error) => {
            tracing::error!(%error, %url, "websocket connect failed");
            return;
        }
    };
    let (mut sink, mut stream) = socket.split();

    let writer = tokio::spawn(async move {
        while let Some(message) = outgoing_rx.recv().await {
            let payload = match serde_json::to_string(&message) {
                Ok(json) => json,
                Err(error) => {
                    tracing::error!(%error, "failed to serialize outgoing message");
                    continue;
                }
            };
            if sink.send(Message::Text(payload.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(frame) = stream.next().await {
        match frame {
            Ok(Message::Text(text)) => match serde_json::from_str::<ServerMessage>(&text) {
                Ok(message) => {
                    if incoming_tx.send(message).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, "malformed server message");
                }
            },
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(_) | Message::Pong(_) | Message::Binary(_) | Message::Frame(_)) => {}
            Err(error) => {
                tracing::warn!(%error, "websocket receive error");
                break;
            }
        }
    }

    drop(incoming_tx);
    let _ = writer.await;
}

/// Builds a `Connector` that accepts any TLS certificate.
///
/// Used only when the user passes `--insecure`. It installs a
/// `ServerCertVerifier` that returns success unconditionally. The verifier
/// is intentionally kept private to this module and is never reachable
/// through the public API of the crate.
fn insecure_connector() -> Connector {
    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoVerifier))
        .with_no_client_auth();
    Connector::Rustls(Arc::new(config))
}

/// Certificate verifier that accepts every certificate.
#[derive(Debug)]
struct NoVerifier;

impl rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        use rustls::SignatureScheme;
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
        ]
    }
}

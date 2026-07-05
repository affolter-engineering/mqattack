use anyhow::{bail, Context, Result};
use clap::Args;
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS, TlsConfiguration, Transport};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error as TlsError, RootCertStore, SignatureScheme};
use std::io::Cursor;
use std::sync::Arc;

/// Connection arguments shared across all commands.
#[derive(Args, Clone, Debug)]
pub struct ConnectionArgs {
    /// MQTT broker hostname or IP address
    #[arg(short = 'H', long, default_value = "localhost")]
    pub host: String,

    /// MQTT broker port
    #[arg(short = 'p', long, default_value_t = 1883)]
    pub port: u16,

    /// Username for authentication
    #[arg(short = 'u', long)]
    pub username: Option<String>,

    /// Password for authentication
    #[arg(short = 'P', long, conflicts_with = "jwt")]
    pub password: Option<String>,

    /// JWT token sent as the MQTT password (username left empty unless -u is also set)
    #[arg(long, conflicts_with = "password")]
    pub jwt: Option<String>,

    /// MQTT client identifier
    #[arg(long, default_value = "mqattack")]
    pub client_id: String,

    /// Keep-alive interval in seconds
    #[arg(long, default_value_t = 60)]
    pub keepalive: u16,

    /// Enable TLS (implied when --cafile, --cert/--key, or --insecure is set)
    #[arg(long)]
    pub tls: bool,

    /// CA certificate file in PEM format (enables TLS)
    #[arg(long, value_name = "PATH")]
    pub cafile: Option<String>,

    /// Client certificate file in PEM format for mTLS (requires --key)
    #[arg(long, value_name = "PATH", requires = "key")]
    pub cert: Option<String>,

    /// Client private key file in PEM format for mTLS (requires --cert)
    #[arg(long, value_name = "PATH", requires = "cert")]
    pub key: Option<String>,

    /// Skip TLS certificate verification — INSECURE, for testing only
    #[arg(long)]
    pub insecure: bool,
}

/// Build an `MqttOptions` from the shared connection arguments.
pub fn build_options(args: &ConnectionArgs) -> Result<MqttOptions> {
    let mut opts = MqttOptions::new(&args.client_id, &args.host, args.port);
    opts.set_keep_alive(std::time::Duration::from_secs(args.keepalive as u64));

    // JWT takes priority over --password; either can be combined with --username.
    let effective_password = args.jwt.as_deref().or(args.password.as_deref());
    match (&args.username, effective_password) {
        (Some(u), Some(p)) => { opts.set_credentials(u, p); }
        (Some(u), None)    => { opts.set_credentials(u, ""); }
        (None, Some(p))    => { opts.set_credentials("", p); }
        _                  => {}
    }

    let use_tls = args.tls || args.cafile.is_some() || args.cert.is_some() || args.insecure;
    if use_tls {
        opts.set_transport(Transport::tls_with_config(build_tls_config(args)?));
    }

    Ok(opts)
}

fn build_tls_config(args: &ConnectionArgs) -> Result<TlsConfiguration> {
    let verifier: Arc<dyn ServerCertVerifier> = if args.insecure {
        eprintln!("[!] WARNING: TLS certificate verification disabled (--insecure)");
        Arc::new(DangerousAcceptAllVerifier)
    } else {
        let mut root_store = RootCertStore::empty();
        if let Some(path) = &args.cafile {
            let bytes = std::fs::read(path)
                .with_context(|| format!("failed to read CA file '{}'", path))?;
            for cert in rustls_pemfile::certs(&mut Cursor::new(&bytes)) {
                root_store.add(cert.context("invalid certificate in CA file")?)?;
            }
        } else {
            for cert in rustls_native_certs::load_native_certs().unwrap_or_default() {
                root_store.add(cert).ok();
            }
        }
        rustls::client::WebPkiServerVerifier::builder(Arc::new(root_store))
            .build()
            .context("failed to build TLS verifier")?
    };

    let builder = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier);

    let config = if let (Some(cert_path), Some(key_path)) = (&args.cert, &args.key) {
        let cert_chain = load_certs(cert_path)?;
        let key = load_key(key_path)?;
        builder
            .with_client_auth_cert(cert_chain, key)
            .context("failed to configure mTLS client certificate")?
    } else {
        builder.with_no_client_auth()
    };

    Ok(TlsConfiguration::Rustls(Arc::new(config)))
}

fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read certificate file '{}'", path))?;
    let certs: std::result::Result<Vec<_>, _> =
        rustls_pemfile::certs(&mut Cursor::new(&bytes)).collect();
    let certs = certs.context("failed to parse certificate file")?;
    if certs.is_empty() {
        bail!("no certificates found in '{}'", path);
    }
    Ok(certs)
}

fn load_key(path: &str) -> Result<PrivateKeyDer<'static>> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read key file '{}'", path))?;
    for item in rustls_pemfile::read_all(&mut Cursor::new(&bytes)) {
        match item.context("failed to parse key file")? {
            rustls_pemfile::Item::Pkcs1Key(k) => return Ok(PrivateKeyDer::Pkcs1(k)),
            rustls_pemfile::Item::Pkcs8Key(k) => return Ok(PrivateKeyDer::Pkcs8(k)),
            rustls_pemfile::Item::Sec1Key(k)  => return Ok(PrivateKeyDer::Sec1(k)),
            _ => {}
        }
    }
    bail!("no private key found in '{}'", path)
}


#[derive(Debug)]
struct DangerousAcceptAllVerifier;

impl ServerCertVerifier for DangerousAcceptAllVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, TlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, TlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, TlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA1,
            SignatureScheme::ECDSA_SHA1_Legacy,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
            SignatureScheme::ED448,
        ]
    }
}

/// Parse a numeric QoS level (0-2) into `rumqttc::QoS`.
pub fn parse_qos(level: u8) -> Result<QoS> {
    match level {
        0 => Ok(QoS::AtMostOnce),
        1 => Ok(QoS::AtLeastOnce),
        2 => Ok(QoS::ExactlyOnce),
        _ => bail!("invalid QoS level: {} (must be 0, 1 or 2)", level),
    }
}

// ── Shared event-loop driver ──────────────────────────────────────────────────

/// Drive `eventloop` until `on_publish` returns `false`, the connection drops,
/// or the caller's enclosing `tokio::select!` branch is cancelled.
///
/// `on_publish` receives each incoming PUBLISH packet and returns `true` to
/// keep running or `false` to stop.
pub async fn poll_loop<F>(eventloop: &mut EventLoop, mut on_publish: F)
where
    F: FnMut(rumqttc::Publish) -> bool,
{
    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::ConnAck(ack))) => {
                eprintln!("[+] Connected (session_present={})", ack.session_present);
            }
            Ok(Event::Incoming(Packet::Publish(p))) => {
                if !on_publish(p) {
                    break;
                }
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("[!] Connection error: {}", e);
                break;
            }
        }
    }
}

/// Convenience: build an `AsyncClient` + `EventLoop` from connection args,
/// subscribe to each topic at the given QoS, and return both handles.
pub async fn connect_and_subscribe(
    args: &ConnectionArgs,
    topics: &[String],
    qos: QoS,
) -> Result<(AsyncClient, EventLoop)> {
    let opts = build_options(args)?;
    let (client, eventloop) = AsyncClient::new(opts, 64);
    for topic in topics {
        client.subscribe(topic, qos).await?;
    }
    Ok((client, eventloop))
}


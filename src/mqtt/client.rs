use anyhow::{bail, Result};
use clap::Args;
use rumqttc::{MqttOptions, QoS};

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
    #[arg(short = 'P', long)]
    pub password: Option<String>,

    /// MQTT client identifier
    #[arg(long, default_value = "mqattack")]
    pub client_id: String,

    /// Keep-alive interval in seconds
    #[arg(long, default_value_t = 60)]
    pub keepalive: u16,
}

/// Build an `MqttOptions` from the shared connection arguments.
pub fn build_options(args: &ConnectionArgs) -> MqttOptions {
    let mut opts = MqttOptions::new(&args.client_id, &args.host, args.port);
    opts.set_keep_alive(std::time::Duration::from_secs(args.keepalive as u64));

    match (&args.username, &args.password) {
        (Some(u), Some(p)) => {
            opts.set_credentials(u, p);
        }
        (Some(u), None) => {
            opts.set_credentials(u, "");
        }
        _ => {}
    }

    opts
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

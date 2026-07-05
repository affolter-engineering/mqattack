use clap::{Parser, Subcommand};

mod commands;
mod mqtt;

#[derive(Parser)]
#[command(name = "mqattack")]
#[command(about = "MQTT penetration testing tool")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Subscribe to one or more topics and print incoming messages
    Subscribe(commands::subscribe::SubscribeArgs),
    /// Publish a message to a topic
    Publish(commands::publish::PublishArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Subscribe(args) => commands::subscribe::run(args).await,
        Commands::Publish(args) => commands::publish::run(args).await,
    }
}

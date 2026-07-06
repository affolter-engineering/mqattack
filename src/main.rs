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
    /// Collect broker metadata from the $SYS/# topic
    SysInfo(commands::sys_info::SysInfoArgs),
    /// Enumerate active topics using a wordlist or brute-force
    EnumTopics(commands::enum_topics::EnumTopicsArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Subscribe(args) => commands::subscribe::run(args).await,
        Commands::Publish(args) => commands::publish::run(args).await,
        Commands::SysInfo(args) => commands::sys_info::run(args).await,
        Commands::EnumTopics(args) => commands::enum_topics::run(args).await,
    }
}

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
    /// Test subscribe/publish ACL permissions for one or more topics
    CheckAcl(commands::check_acl::CheckAclArgs),
    /// Test subscribe/publish permissions across multiple users and build a permission matrix
    EnumPerms(commands::enum_perms::EnumPermsArgs),
    /// Publish a series of injection payloads to a topic to probe downstream systems
    PayloadInject(commands::payload_inject::PayloadInjectArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Subscribe(args) => commands::subscribe::run(args).await,
        Commands::Publish(args) => commands::publish::run(args).await,
        Commands::SysInfo(args) => commands::sys_info::run(args).await,
        Commands::EnumTopics(args) => commands::enum_topics::run(args).await,
        Commands::CheckAcl(args) => commands::check_acl::run(args).await,
        Commands::EnumPerms(args) => commands::enum_perms::run(args).await,
        Commands::PayloadInject(args) => commands::payload_inject::run(args).await,
    }
}

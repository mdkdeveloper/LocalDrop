//! `localdrop` — command-line front-end, mainly for development and testing.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use localdrop_core::{DeviceId, GroupConfig, GroupKey, KdfParams};

#[derive(Parser)]
#[command(
    name = "localdrop",
    version,
    about = "Send files between your devices on the LAN"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Join (or create) a group by name and shared password.
    Init {
        /// Group name, e.g. "home".
        #[arg(long)]
        group: String,
        /// Shared group password. Omit to be prompted (not implemented yet).
        #[arg(long)]
        password: String,
        /// Name other devices will see.
        #[arg(long)]
        name: Option<String>,
    },
    /// List group members (online and known).
    Peers,
    /// Send files to an online device now.
    Send {
        /// Target device id.
        device: DeviceId,
        /// Files to send.
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Queue files for a known device; delivered when it comes online.
    Queue {
        /// Target device id.
        device: DeviceId,
        /// Files to queue.
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Show the outbox.
    Outbox,
    /// Run in the foreground: announce, receive, and deliver queued transfers.
    Run,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> localdrop_core::Result<()> {
    match cli.command {
        Command::Init {
            group,
            password,
            name,
        } => {
            let cfg = GroupConfig::new(&group)?;
            let key = GroupKey::derive(&cfg, &password, KdfParams::V1)?;
            println!("group:       {}", cfg.name());
            println!(
                "fingerprint: {}  (must match on all your devices)",
                key.fingerprint()
            );
            if let Some(name) = name {
                println!("device name: {name}");
            }
            println!("note: saving configuration is not implemented yet");
            Ok(())
        }
        Command::Peers
        | Command::Send { .. }
        | Command::Queue { .. }
        | Command::Outbox
        | Command::Run => Err(localdrop_core::Error::NotImplemented(
            "this command needs persistent config and networking (see docs/roadmap.md)",
        )),
    }
}

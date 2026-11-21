//! `localdrop` — command-line front-end, mainly for development and testing.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, Subcommand};
use localdrop_core::{DeviceId, KdfParams};
use localdrop_discovery::net::{list_interfaces, network_id};
use localdrop_node::{NetworkTrust, Node, NodeEvent, Presence, Settings, StateDir};
use localdrop_store::MemoryStore;

#[derive(Parser)]
#[command(
    name = "localdrop",
    version,
    about = "Send files between your devices on the LAN"
)]
struct Cli {
    /// State folder (default: $LOCALDROP_HOME or the per-user config folder).
    #[arg(long, global = true)]
    home: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Join (or create) a group by name and shared password, and save the state.
    Init {
        /// Group name, e.g. "home".
        #[arg(long)]
        group: String,
        /// Shared group password (or set LOCALDROP_PASSWORD).
        #[arg(long, env = "LOCALDROP_PASSWORD", hide_env_values = true)]
        password: String,
        /// Name other devices will see (default: this computer's name).
        #[arg(long)]
        name: Option<String>,
        /// Don't trust the network this computer is on now.
        #[arg(long)]
        no_trust: bool,
        /// Replace an existing group (the device identity is kept).
        #[arg(long)]
        force: bool,
    },
    /// Run in the foreground: discover group members and print what happens.
    Run,
    /// Look for group members for a few seconds and list them.
    Peers {
        /// Seconds to listen.
        #[arg(long, default_value_t = 7)]
        wait: u64,
    },
    /// List connected and remembered networks and how LocalDrop treats them.
    Networks,
    /// Trust a network (by key from `networks`, or `current`), or stop trusting it.
    Trust {
        /// Network key (at least 4 hex characters) or `current`.
        network: String,
        /// Mark it not trusted (asked again next time).
        #[arg(long, conflicts_with = "never")]
        not: bool,
        /// Not trusted, and never ask about it again.
        #[arg(long)]
        never: bool,
    },
    /// Wait until a group member is online and measure the round trip over its
    /// authenticated connection.
    Ping {
        /// Device id, the start of one, or its name.
        device: String,
        /// Pings to send.
        #[arg(long, default_value_t = 3)]
        count: u32,
        /// Seconds to wait for the device to come online.
        #[arg(long, default_value_t = 15)]
        wait: u64,
    },
    /// Connect to a device by address (`ip[:port]` or a name) when discovery can't find
    /// it. Without a port, the other device must listen on the fixed port 47474.
    Connect {
        /// Address, e.g. `192.168.1.42:51234` or `desk-pc.local`.
        address: String,
        /// Try this address again whenever LocalDrop starts.
        #[arg(long)]
        remember: bool,
    },
    /// Change the group password. Devices that don't get the new one drop out.
    Password {
        /// Current password (or set LOCALDROP_PASSWORD).
        #[arg(long, env = "LOCALDROP_PASSWORD", hide_env_values = true)]
        current: String,
        /// New password, at least 10 characters (or set LOCALDROP_NEW_PASSWORD).
        #[arg(long, env = "LOCALDROP_NEW_PASSWORD", hide_env_values = true)]
        new: String,
    },
    /// Leave the group: deletes the group key; files and trusted networks stay.
    Leave {
        /// Cancel the jobs queued for other devices (kept by default).
        #[arg(long)]
        drop_queue: bool,
        /// Also forget known devices and their pinned keys.
        #[arg(long)]
        forget_devices: bool,
    },
    /// Send files to an online device now (M3).
    Send {
        /// Target device id.
        device: DeviceId,
        /// Files and folders to send.
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Queue files for a known device; delivered when it comes online (M5).
    Queue {
        /// Target device id.
        device: DeviceId,
        /// Files and folders to queue.
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Show the job queues and history (M3).
    Jobs,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();
    let dir = StateDir::new(cli.home.clone().unwrap_or_else(StateDir::default_location));
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    match rt.block_on(run(cli.command, dir)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn open(
    dir: &StateDir,
) -> localdrop_core::Result<(Node, tokio::sync::mpsc::UnboundedReceiver<NodeEvent>)> {
    if !dir.is_joined() {
        eprintln!("Not in a group yet. Run `localdrop init --group <name>` first.");
        return Err(localdrop_core::Error::NotFound);
    }
    Node::open(dir.clone(), Arc::new(MemoryStore::new()))
}

fn computer_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "My computer".into())
}

async fn run(cmd: Command, dir: StateDir) -> localdrop_core::Result<()> {
    match cmd {
        Command::Init {
            group,
            password,
            name,
            no_trust,
            force,
        } => {
            if dir.is_joined() && !force {
                eprintln!(
                    "Already in a group (state in {}). Use --force to replace it.",
                    dir.path().display()
                );
                return Err(localdrop_core::Error::InvalidState);
            }
            if password.chars().count() < 10 {
                eprintln!("The password must have at least 10 characters (docs/security.md).");
                return Err(localdrop_core::Error::InvalidPassword("too short"));
            }
            let name = name.unwrap_or_else(computer_name);
            println!("Deriving the group key…");
            let joined = dir.init(
                &group,
                &password,
                |g| Settings::new(name.clone(), g.name()),
                KdfParams::V1,
            )?;
            let mut settings = joined.settings;
            let mut trusted = Vec::new();
            if !no_trust {
                // "Trust this network": the networks this computer is on now (those with
                // a default gateway, i.e. the real LAN).
                for i in list_interfaces() {
                    if i.class.usable_by_default() && i.gateway_ip.is_some() {
                        let id = network_id(&i);
                        settings.set_trust(
                            id.key,
                            &id.default_label,
                            id.kind,
                            id.weak,
                            NetworkTrust::Trusted,
                        );
                        trusted.push(id.default_label);
                    }
                }
                dir.save_settings(&settings)?;
            }
            println!("group:       {}", joined.group.name());
            println!(
                "fingerprint: {}  (must match on all your devices)",
                joined.key.fingerprint()
            );
            println!(
                "device:      {} ({})",
                settings.device_name,
                joined.identity.device_id()
            );
            if trusted.is_empty() {
                println!("trusted:     none yet; see `localdrop networks` and `localdrop trust`");
            } else {
                println!("trusted:     {}", trusted.join(", "));
            }
            println!("state:       {}", dir.path().display());
            Ok(())
        }

        Command::Run => {
            let (node, mut events) = open(&dir)?;
            let info = node.group_info();
            println!(
                "{} in group {} ({}) as {}. Ctrl+C to stop.",
                node.settings().device_name,
                info.name,
                info.fingerprint,
                node.device_id()
            );
            node.start().await?;
            if let Some(port) = node.listen_port() {
                println!("listening on UDP port {port}");
            }
            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => break,
                    Some(ev) = events.recv() => print_event(&node, &ev),
                }
            }
            node.stop().await?;
            Ok(())
        }

        Command::Peers { wait } => {
            let (node, mut events) = open(&dir)?;
            node.start().await?;
            if !node.networks()?.iter().any(|n| n.active) {
                println!("Discovery is off on every connected network: none is trusted.");
                println!("Trust one with `localdrop trust current`.");
            }
            let deadline = tokio::time::sleep(Duration::from_secs(wait));
            tokio::pin!(deadline);
            loop {
                tokio::select! {
                    _ = &mut deadline => break,
                    Some(_) = events.recv() => {}
                }
            }
            let found: Vec<_> = node
                .devices()?
                .into_iter()
                .filter(|d| d.presence != Presence::Offline)
                .collect();
            if found.is_empty() {
                println!("No group members found in {wait} s.");
            }
            for d in found {
                let addrs: Vec<_> = node
                    .peer_addrs(d.id)
                    .iter()
                    .map(ToString::to_string)
                    .collect();
                let state = if d.presence == Presence::Online {
                    "online"
                } else {
                    "found, not connected"
                };
                println!("{}  {}  {state}  {}", d.id, d.name, addrs.join(", "));
            }
            node.stop().await?;
            Ok(())
        }

        Command::Networks => {
            let (node, _) = open(&dir)?;
            let nets = node.networks()?;
            if nets.is_empty() {
                println!("No Wi-Fi or Ethernet network connected.");
            }
            for n in &nets {
                let state = match (n.connected, n.active, n.trust) {
                    (true, true, _) => "trusted · discovering",
                    (true, false, NetworkTrust::Trusted) => "trusted · not selected",
                    (_, _, NetworkTrust::Trusted) => "trusted",
                    (_, _, NetworkTrust::NotTrusted) => "not trusted",
                    (_, _, NetworkTrust::NeverAsk) => "not trusted · never ask",
                };
                println!(
                    "{}  {:<28} {}{}{}",
                    &n.key[..8],
                    n.label,
                    state,
                    if n.connected { "" } else { " · not connected" },
                    if n.weak_identity {
                        " · known by address only"
                    } else {
                        ""
                    },
                );
            }
            for i in list_interfaces() {
                if !i.class.usable_by_default() {
                    println!("          {:<28} {:?} · ignored", i.label, i.class);
                }
            }
            Ok(())
        }

        Command::Trust {
            network,
            not,
            never,
        } => {
            let (node, _) = open(&dir)?;
            let key = if network == "current" {
                let nets = node.networks()?;
                let connected: Vec<_> = nets.iter().filter(|n| n.connected).collect();
                match connected.as_slice() {
                    [one] => one.key.clone(),
                    [] => {
                        eprintln!("No Wi-Fi or Ethernet network connected.");
                        return Err(localdrop_core::Error::NotFound);
                    }
                    _ => {
                        eprintln!(
                            "Several networks are connected; pick one from `localdrop networks`."
                        );
                        return Err(localdrop_core::Error::InvalidState);
                    }
                }
            } else {
                network
            };
            let trust = if never {
                NetworkTrust::NeverAsk
            } else if not {
                NetworkTrust::NotTrusted
            } else {
                NetworkTrust::Trusted
            };
            node.set_network_trust(key.clone(), trust).await?;
            let n = node
                .networks()?
                .into_iter()
                .find(|n| n.key.starts_with(&key.to_ascii_lowercase()))
                .expect("just updated");
            println!("{}: {:?}", n.label, n.trust);
            if n.weak_identity && trust == NetworkTrust::Trusted {
                println!(
                    "Note: this network is known by its address only; other networks with the same address will be trusted too."
                );
            }
            Ok(())
        }

        Command::Ping {
            device,
            count,
            wait,
        } => {
            let (node, mut events) = open(&dir)?;
            node.start().await?;
            let deadline = tokio::time::sleep(Duration::from_secs(wait));
            tokio::pin!(deadline);
            let target = loop {
                if let Some(d) = find_online(&node, &device)? {
                    break d;
                }
                tokio::select! {
                    _ = &mut deadline => {
                        node.stop().await?;
                        return Err(localdrop_core::Error::Transport(format!(
                            "“{device}” didn't come online in {wait} s (see `localdrop peers`)"
                        )));
                    }
                    Some(ev) = events.recv() => {
                        if let NodeEvent::KeyMismatch { device, name } = &ev {
                            eprintln!("{name} ({device}) presented another key and was refused");
                        }
                    }
                }
            };
            println!("{} ({}) is online", target.1, target.0);
            for i in 0..count {
                match node.ping(target.0).await {
                    Ok(rtt) => println!("pong {}: {:.1} ms", i + 1, rtt.as_secs_f64() * 1000.0),
                    Err(e) => println!("ping {}: {e}", i + 1),
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            node.stop().await?;
            Ok(())
        }

        Command::Connect { address, remember } => {
            let (node, _events) = open(&dir)?;
            node.start().await?;
            let r = node.connect_address(address.clone(), remember).await;
            let out = match r {
                Ok(id) => {
                    let name = node
                        .devices()?
                        .into_iter()
                        .find(|d| d.id == id)
                        .map_or_else(String::new, |d| d.name);
                    println!("connected to {name} ({id})");
                    let rtt = node.ping(id).await?;
                    println!("round trip {:.1} ms", rtt.as_secs_f64() * 1000.0);
                    if remember {
                        println!("remembered {address}");
                    }
                    Ok(())
                }
                Err(e) => Err(e),
            };
            node.stop().await?;
            out
        }

        Command::Password { current, new } => {
            let (node, _) = open(&dir)?;
            let before = node.group_info().fingerprint;
            println!("checking the current password…");
            let info = node.change_password(current, new).await?;
            println!("password changed for group {}", info.name);
            println!("fingerprint: {before} → {}", info.fingerprint);
            println!("Enter the new password on every device you keep; the others drop out.");
            Ok(())
        }

        Command::Leave {
            drop_queue,
            forget_devices,
        } => {
            let (node, _) = open(&dir)?;
            let info = node.group_info();
            node.leave_group(!drop_queue, forget_devices).await?;
            println!("left group {} ({})", info.name, info.fingerprint);
            println!("Join again with `localdrop init --group <name>`.");
            Ok(())
        }

        Command::Send { .. } | Command::Queue { .. } | Command::Jobs => Err(
            localdrop_core::Error::NotImplemented("sending needs the transfer engine (M3)"),
        ),
    }
}

/// An online device by id, id prefix (4+ characters) or name (case-insensitive).
fn find_online(node: &Node, query: &str) -> localdrop_core::Result<Option<(DeviceId, String)>> {
    let q = query.trim().to_lowercase();
    Ok(node
        .devices()?
        .into_iter()
        .filter(|d| d.presence == Presence::Online)
        .find(|d| d.name.to_lowercase() == q || (q.len() >= 4 && d.id.to_string().starts_with(&q)))
        .map(|d| (d.id, d.name)))
}

fn print_event(node: &Node, ev: &NodeEvent) {
    let t = chrono_like_now();
    match ev {
        NodeEvent::DiscoveryChanged { on: false, .. } => println!("{t} discovery off"),
        NodeEvent::DiscoveryChanged {
            on: true,
            active_networks: 0,
        } => println!(
            "{t} discovery on, but no trusted network is connected (`localdrop trust current`)"
        ),
        NodeEvent::DiscoveryChanged {
            on: true,
            active_networks: n,
        } => println!("{t} discovering on {n} trusted network(s)"),
        NodeEvent::UntrustedNetwork { key, label } => println!(
            "{t} connected to “{label}”, which isn't trusted; `localdrop trust {}` to search there",
            &key[..8]
        ),
        NodeEvent::PeerFound { device } => {
            let addrs: Vec<_> = node
                .peer_addrs(*device)
                .iter()
                .map(ToString::to_string)
                .collect();
            println!("{t} found {device} at {}", addrs.join(", "));
        }
        NodeEvent::PeerOnline { device, name } => {
            println!("{t} {name} ({device}) is online")
        }
        NodeEvent::KeyMismatch { device, name } => {
            println!("{t} {name} ({device}) presented a different key and was refused")
        }
        NodeEvent::PeerOffline { device } => println!("{t} lost {device}"),
        other => println!("{t} {other:?}"),
    }
}

/// `HH:MM:SS` in UTC, without pulling in a date crate.
fn chrono_like_now() -> String {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

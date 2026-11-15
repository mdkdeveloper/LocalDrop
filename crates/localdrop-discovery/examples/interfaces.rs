//! Prints how this machine's interfaces are classified and fingerprinted:
//! `cargo run -p localdrop-discovery --example interfaces`.

use localdrop_discovery::net::{list_interfaces, network_id};

fn main() {
    for i in list_interfaces() {
        let id = network_id(&i);
        let subnets: Vec<_> = i
            .subnets
            .iter()
            .map(|s| format!("{}={}", s.addr, s))
            .collect();
        println!(
            "{:<10} {:<40} {}  gw {:?} mac {}  net {} ({}{})",
            format!("{:?}", i.class),
            i.label,
            subnets.join(" "),
            i.gateway_ip,
            if i.gateway_mac.is_some() { "yes" } else { "no" },
            id.key,
            id.default_label,
            if id.weak { ", weak" } else { "" },
        );
    }
}

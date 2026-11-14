//! QUIC transport (stub).

use std::net::SocketAddr;

use localdrop_core::{Error, Result};

use crate::Transport;

/// QUIC-based [`Transport`]. Not implemented yet.
#[derive(Debug, Default)]
pub struct QuicTransport;

impl Transport for QuicTransport {
    fn local_addr(&self) -> Result<SocketAddr> {
        Err(Error::NotImplemented("QUIC transport"))
    }
}

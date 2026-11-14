//! Length-prefixed `postcard` framing: `u32 BE length || postcard(message)`.

use localdrop_core::{Error, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Maximum size of a single frame payload (1 MiB). Larger frames are a protocol error.
pub const MAX_FRAME_LEN: usize = 1024 * 1024;

/// Encodes a message into a frame.
pub fn encode_frame<T: Serialize>(msg: &T) -> Result<Vec<u8>> {
    let body = postcard::to_stdvec(msg).map_err(|e| Error::Protocol(e.to_string()))?;
    if body.len() > MAX_FRAME_LEN {
        return Err(Error::Protocol("frame too large".into()));
    }
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Tries to decode one frame from the start of `buf`.
///
/// Returns `Ok(None)` if more bytes are needed, or `Ok(Some((msg, consumed)))`.
pub fn decode_frame<T: DeserializeOwned>(buf: &[u8]) -> Result<Option<(T, usize)>> {
    let Some(len_bytes) = buf.get(..4) else {
        return Ok(None);
    };
    let len = u32::from_be_bytes(len_bytes.try_into().expect("4 bytes")) as usize;
    if len > MAX_FRAME_LEN {
        return Err(Error::Protocol("frame too large".into()));
    }
    let Some(body) = buf.get(4..4 + len) else {
        return Ok(None);
    };
    let msg = postcard::from_bytes(body).map_err(|e| Error::Protocol(e.to_string()))?;
    Ok(Some((msg, 4 + len)))
}

#[cfg(test)]
mod tests {
    use localdrop_core::{DeviceId, TransferId};

    use super::*;
    use crate::{ControlMessage, FileMeta};

    #[test]
    fn roundtrip() {
        let msgs = vec![
            ControlMessage::Hello {
                protocol_version: 1,
                device_id: DeviceId([3; 16]),
                device_name: "laptop".into(),
            },
            ControlMessage::Offer {
                transfer_id: TransferId([9; 16]),
                files: vec![FileMeta {
                    index: 0,
                    name: "a.txt".into(),
                    size: 5,
                    blake3: [1; 32],
                }],
            },
            ControlMessage::Ping,
        ];
        let mut buf = Vec::new();
        for m in &msgs {
            buf.extend(encode_frame(m).unwrap());
        }
        let mut pos = 0;
        let mut decoded = Vec::new();
        while let Some((m, n)) = decode_frame::<ControlMessage>(&buf[pos..]).unwrap() {
            decoded.push(m);
            pos += n;
        }
        assert_eq!(decoded, msgs);
    }

    #[test]
    fn partial_needs_more() {
        let frame = encode_frame(&ControlMessage::Pong).unwrap();
        assert!(
            decode_frame::<ControlMessage>(&frame[..2])
                .unwrap()
                .is_none()
        );
        assert!(
            decode_frame::<ControlMessage>(&frame[..frame.len() - 1])
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn oversize_rejected() {
        let buf = (u32::MAX).to_be_bytes();
        assert!(decode_frame::<ControlMessage>(&buf).is_err());
    }
}

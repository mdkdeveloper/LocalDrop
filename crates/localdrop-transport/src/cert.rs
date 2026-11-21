//! Self-signed device certificates and the TLS verifiers that accept them.
//!
//! Each device presents an X.509 certificate signed by its own Ed25519 device key. There
//! is no CA: TLS only proves that the peer holds the private key of the public key in its
//! certificate. Whether that key belongs to a group member is decided by the membership
//! [`handshake`](crate::handshake), and which device it is by its [`DeviceId`]
//! (derived from the key).

use std::sync::Arc;

use ed25519_dalek::pkcs8::EncodePrivateKey;
use localdrop_core::{DeviceIdentity, Error, PublicKey, Result};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, WebPkiSupportedAlgorithms, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{DigitallySignedStruct, DistinguishedName, SignatureScheme};

/// DER prefix of an Ed25519 `SubjectPublicKeyInfo`; the 32 key bytes follow.
const ED25519_SPKI_PREFIX: [u8; 12] = [
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
];

/// Name put in the certificate and sent as SNI; never checked.
pub(crate) const SERVER_NAME: &str = "localdrop";

/// Builds this device's certificate and private key for rustls.
pub fn device_certificate(
    identity: &DeviceIdentity,
) -> Result<(CertificateDer<'static>, PrivateKeyDer<'static>)> {
    let pkcs8 = identity
        .signing_key()
        .to_pkcs8_der()
        .map_err(|e| Error::Transport(format!("device key: {e}")))?;
    let der = PrivatePkcs8KeyDer::from(pkcs8.as_bytes().to_vec());
    let key_pair = rcgen::KeyPair::from_pkcs8_der_and_sign_algo(&der, &rcgen::PKCS_ED25519)
        .map_err(|e| Error::Transport(format!("device key: {e}")))?;
    let params = rcgen::CertificateParams::new(vec![SERVER_NAME.to_owned()])
        .map_err(|e| Error::Transport(format!("certificate: {e}")))?;
    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| Error::Transport(format!("certificate: {e}")))?;
    Ok((cert.der().clone(), PrivateKeyDer::Pkcs8(der)))
}

/// The Ed25519 public key in a certificate, if it holds exactly one.
pub fn certificate_key(cert: &[u8]) -> Option<PublicKey> {
    let mut found = None;
    for i in 0..cert.len().saturating_sub(ED25519_SPKI_PREFIX.len() + 31) {
        if cert[i..].starts_with(&ED25519_SPKI_PREFIX) {
            if found.is_some() {
                return None;
            }
            let start = i + ED25519_SPKI_PREFIX.len();
            let key: [u8; 32] = cert[start..start + 32].try_into().ok()?;
            found = Some(PublicKey(key));
        }
    }
    found
}

/// The crypto provider used everywhere (ring).
pub(crate) fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn check_cert(cert: &CertificateDer<'_>) -> std::result::Result<(), rustls::Error> {
    certificate_key(cert)
        .map(|_| ())
        .ok_or(rustls::Error::InvalidCertificate(
            rustls::CertificateError::BadEncoding,
        ))
}

/// Accepts any certificate that carries an Ed25519 key and a valid TLS 1.3 signature.
#[derive(Debug)]
pub(crate) struct DeviceCertVerifier {
    algs: WebPkiSupportedAlgorithms,
}

impl DeviceCertVerifier {
    pub fn new(provider: &CryptoProvider) -> Arc<Self> {
        Arc::new(Self {
            algs: provider.signature_verification_algorithms,
        })
    }

    fn tls13(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        if dss.scheme != SignatureScheme::ED25519 {
            return Err(rustls::Error::PeerMisbehaved(
                rustls::PeerMisbehaved::SignedHandshakeWithUnadvertisedSigScheme,
            ));
        }
        verify_tls13_signature(message, cert, dss, &self.algs)
    }
}

impl ServerCertVerifier for DeviceCertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        check_cert(end_entity)?;
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Err(rustls::Error::PeerIncompatible(
            rustls::PeerIncompatible::Tls13RequiredForQuic,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        self.tls13(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }
}

impl ClientCertVerifier for DeviceCertVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> std::result::Result<ClientCertVerified, rustls::Error> {
        check_cert(end_entity)?;
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Err(rustls::Error::PeerIncompatible(
            rustls::PeerIncompatible::Tls13RequiredForQuic,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        self.tls13(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }

    fn client_auth_mandatory(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certificate_carries_the_device_key() {
        let id = DeviceIdentity::generate();
        let (cert, _) = device_certificate(&id).unwrap();
        assert_eq!(certificate_key(&cert), Some(id.public_key()));
        assert_eq!(certificate_key(b"not a certificate"), None);
    }
}

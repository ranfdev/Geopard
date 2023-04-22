#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq, Eq)]
pub enum CertificateError {
    #[error("First time seen")]
    FirstUse,
    #[error("Certificate is expired")]
    Expired,
    #[error("Certificate is revoked")]
    Revoked,
    #[error("Certificate is not activated yet")]
    NotActivated,
    #[error("Certificate is not valid for this host")]
    BadIdentity,
    #[error("Generic certificate error")]
    GenericError,
}
pub struct Certificate<'a>(&'a str);
pub trait CertProvider: std::fmt::Debug {
    fn validate(&self, host: &str, cert: &gio::TlsCertificate) -> Result<(), CertificateError>;
    fn override_temp_validity(&self, host: &str);
    fn remove_cert(&self, host: &str);
}

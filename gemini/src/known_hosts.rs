use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, Write};

use gio::prelude::*;

#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq, Eq)]
pub enum CertificateError {
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
pub trait KnownHostsRepo: std::fmt::Debug {
    fn get(&self, host: &str) -> Option<&str>;
    fn insert(&mut self, host: &str, sha: &str) -> bool;
    fn remove(&mut self, host: &str) -> bool;
    fn values(&self) -> HashMap<String, String>;
}

pub fn validate(
    repo: &mut impl KnownHostsRepo,
    host: &str,
    cert: &gio::TlsCertificate,
) -> Result<(), CertificateError> {
    let cert_sha = {
        let mut ck = glib::Checksum::new(glib::ChecksumType::Sha256).unwrap();
        ck.update(&cert.certificate().unwrap());
        ck.string().unwrap()
    };
    {
        if let Some(known_host_sha) = repo.get(host) {
            if known_host_sha != cert_sha {
                return Err(CertificateError::BadIdentity);
            }
            if let Some(cert_end_date) = cert.not_valid_after() {
                if cert_end_date < glib::DateTime::now_utc().unwrap() {
                    return Err(CertificateError::Expired);
                }
            }
            if let Some(cert_start_date) = cert.not_valid_before() {
                if cert_start_date > glib::DateTime::now_utc().unwrap() {
                    return Err(CertificateError::NotActivated);
                }
            }

            return Ok(());
        }
    }
    repo.insert(host, &cert_sha);
    validate(repo, host, cert)
}

#[derive(Debug, Clone, Default)]
pub struct KnownHostsMap(HashMap<String, String>);
impl KnownHostsMap {
    pub fn new() -> Self {
        Self::default()
    }
}
impl KnownHostsRepo for KnownHostsMap {
    fn get(&self, host: &str) -> Option<&str> {
        self.0.get(host).map(|s| s.as_str())
    }

    fn insert(&mut self, host: &str, sha: &str) -> bool {
        self.0.insert(host.to_string(), sha.to_string()).is_none()
    }

    fn remove(&mut self, host: &str) -> bool {
        self.0.remove(host).is_some()
    }
    fn values(&self) -> HashMap<String, String> {
        self.0.clone()
    }
}

#[derive(Debug)]
pub struct KnownHostsFile {
    file: fs::File,
    known_hosts: KnownHostsMap,
}

impl KnownHostsFile {
    pub fn new(file: fs::File) -> Self {
        let mut known_hosts = KnownHostsMap::new();
        let mut bf = std::io::BufReader::new(file);
        let mut lines = String::new();
        bf.read_to_string(&mut lines).unwrap();
        lines.split('\n').for_each(|line| {
            let mut parts = line.split(' ');
            if let (Some(host), Some(sha)) = (parts.next(), parts.next()) {
                known_hosts.insert(host, sha);
            }
        });
        let file = bf.into_inner();
        Self { file, known_hosts }
    }
}

impl KnownHostsRepo for KnownHostsFile {
    fn get(&self, host: &str) -> Option<&str> {
        self.known_hosts.get(host)
    }

    fn insert(&mut self, host: &str, sha: &str) -> bool {
        let new = self.known_hosts.insert(host, sha);
        self.file
            .write_all(format!("{host} {sha}\n").as_bytes())
            .unwrap();
        new
    }

    fn remove(&mut self, host: &str) -> bool {
        let r = self.known_hosts.remove(host);
        self.file.set_len(0).unwrap();
        self.file.rewind().unwrap();
        for (host, sha) in self.known_hosts.values() {
            self.file
                .write_all(format!("{host} {sha}\n").as_bytes())
                .unwrap();
        }
        r
    }

    fn values(&self) -> HashMap<String, String> {
        self.known_hosts.values()
    }
}

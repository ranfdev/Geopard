use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::{fs, path};

use anyhow::Context;
use gemini::CertificateError;
use gtk::prelude::*;
use gtk::{gio, glib};

use crate::common;

#[derive(Debug, Clone)]
struct KnownHostCert {
    sha: String,
    session_override: bool,
}

#[derive(Debug, Clone)]
pub struct KnownHosts {
    file_path: path::PathBuf,
    known_hosts: RefCell<HashMap<String, KnownHostCert>>,
}
impl KnownHosts {
    fn parse_line(line: &str) -> Option<(String, KnownHostCert)> {
        let mut parts = line.split(' ');
        let host = parts.next()?;
        let sha = parts.next()?;
        Some((
            host.to_string(),
            KnownHostCert {
                sha: sha.to_string(),
                session_override: false,
            },
        ))
    }
    fn from_path(path: &std::path::Path) -> anyhow::Result<Self> {
        let known_hosts = if !path.exists() {
            HashMap::new()
        } else {
            fs::read_to_string(path)
                .with_context(|| format!("opening file at {:?}", path))?
                .lines()
                .map(|line| Self::parse_line(line).context("parsing line"))
                .collect::<anyhow::Result<HashMap<_, _>>>()?
        };

        Ok(Self {
            known_hosts: RefCell::new(known_hosts),
            file_path: path.into(),
        })
    }
    fn add_cert(&self, host: &str, cert: gio::TlsCertificate) {
        let cert_sha = {
            let mut ck = glib::Checksum::new(glib::ChecksumType::Sha256).unwrap();
            ck.update(&cert.certificate().unwrap());
            ck.string().unwrap()
        };

        let mut known_hosts_file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(&self.file_path)
            .unwrap();

        writeln!(known_hosts_file, "{} {}", &host, &cert_sha).unwrap();

        self.known_hosts.borrow_mut().insert(
            host.to_string(),
            KnownHostCert {
                sha: cert_sha,
                session_override: false,
            },
        );
    }
}
impl Default for KnownHosts {
    fn default() -> Self {
        Self::from_path(&common::KNOWN_HOSTS_PATH).unwrap()
    }
}

impl gemini::CertProvider for KnownHosts {
    fn validate(&self, host: &str, cert: &gio::TlsCertificate) -> Result<(), CertificateError> {
        {
            if let Some(known_host) = self.known_hosts.borrow().get(host) {
                if known_host.session_override {
                    return Ok(());
                }
                let cert_sha = {
                    let mut ck = glib::Checksum::new(glib::ChecksumType::Sha256).unwrap();
                    ck.update(&cert.certificate().unwrap());
                    ck.string().unwrap()
                };

                if known_host.sha != cert_sha {
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
        self.add_cert(host, cert.clone());
        self.validate(host, cert)
    }

    fn override_temp_validity(&self, host: &str) {
        if let Some(known_host) = self.known_hosts.borrow_mut().get_mut(host) {
            known_host.session_override = true;
        }
    }

    fn remove_cert(&self, host: &str) {
        let mut known_hosts = self.known_hosts.borrow_mut();
        known_hosts.remove(host);

        let mut known_hosts_file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.file_path)
            .unwrap();

        for (host, cert) in known_hosts.iter() {
            writeln!(known_hosts_file, "{} {}", &host, &cert.sha).unwrap();
        }
    }
}

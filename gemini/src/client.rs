use std::convert::TryFrom;

use async_net::TcpStream;
use futures::io::Cursor;
use futures::prelude::*;
use log::debug;
use url::Url;

const INIT_BUFFER_SIZE: usize = 8192; // 8Kb
const MAX_REDIRECT: u8 = 5;

#[derive(Debug, thiserror::Error)]
pub enum ProtoError {
    #[error("Invalid status")]
    InvalidStatus(#[from] InvalidStatus),
    #[error("Meta not found (no <CR><CF>)")]
    MetaNotFound,
}

#[derive(Debug, thiserror::Error)]
#[error("Invalid status")]
pub struct InvalidStatus;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Io error: {0:?}")]
    Io(#[from] std::io::Error),
    #[error("The server doesn't follow the gemini protocol: {0:?}")]
    InvalidProtocolData(#[from] ProtoError),
    #[error("The server sent invalid utf8: {0:?}")]
    InvalidUtf8(#[from] std::str::Utf8Error),
    #[error("Invalid url: {0:?}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("Tls error: {0:?}")]
    Tls(#[from] async_native_tls::Error),
    #[error("Invalid host")]
    InvalidHost,
    #[error("Too many redirections. Last requested redirect was {0}")]
    TooManyRedirects(String),
    #[error("This library only support the gemini url scheme")]
    SchemeNotSupported,
}

#[derive(Debug, Copy, Clone, PartialOrd, PartialEq, Eq)]
pub enum Status {
    Input(u8),
    Success(u8),
    Redirect(u8),
    TempFail(u8),
    PermFail(u8),
    CertRequired(u8),
}
impl TryFrom<u8> for Status {
    type Error = InvalidStatus;
    fn try_from(s: u8) -> Result<Self, Self::Error> {
        match s / 10 {
            1 => Ok(Status::Input(s)),
            2 => Ok(Status::Success(s)),
            3 => Ok(Status::Redirect(s)),
            4 => Ok(Status::TempFail(s)),
            5 => Ok(Status::PermFail(s)),
            6 => Ok(Status::CertRequired(s)),
            _ => Err(InvalidStatus),
        }
    }
}

#[derive(Debug)]
pub struct Response {
    cursor: Cursor<Vec<u8>>,
    status: Status,
    meta: String,
    tls_s: async_native_tls::TlsStream<TcpStream>,
}
impl Response {
    pub fn status(&self) -> Status {
        self.status
    }
    pub fn meta(&self) -> &str {
        &self.meta
    }
    pub fn meta_owned(self) -> String {
        self.meta
    }
    pub fn body(self) -> Option<impl AsyncRead> {
        match self.status {
            Status::Success(_) => Some(self.cursor.chain(self.tls_s)),
            _ => None,
        }
    }
}
#[derive(Default, PartialEq, Eq, Debug, Copy, Clone)]
pub struct ClientOptions {
    redirect: bool,
}

#[derive(Default, Debug, Clone)]
pub struct ClientBuilder {
    options: ClientOptions,
}

impl ClientBuilder {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn redirect(mut self, redirect: bool) -> Self {
        self.options.redirect = redirect;
        self
    }
    pub fn build(self) -> Client {
        Client {
            options: self.options,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Client {
    options: ClientOptions,
}
impl Client {
    pub fn new() -> Self {
        Self {
            options: Default::default(),
        }
    }
    pub async fn fetch(&self, url_str: &str) -> Result<Response, Error> {
        let mut url_str = url_str;
        let mut res;
        let mut i = 0;
        let max_redirect = if self.options.redirect {
            MAX_REDIRECT
        } else {
            1
        };
        while {
            let url = Url::parse(url_str)?;
            res = Self::fetch_internal(url).await?;
            i < max_redirect
        } {
            match res.status() {
                Status::Redirect(_) => {
                    url_str = res.meta();
                    i += 1;
                }
                _ => break,
            }
        }
        Ok(res)
    }
    async fn fetch_internal(url: Url) -> Result<Response, Error> {
        if url.scheme() != "gemini" {
            return Err(Error::SchemeNotSupported);
        }

        let port = url.port().unwrap_or(1965);
        let host = url.host_str().ok_or(Error::InvalidHost)?;
        let addr = async_net::resolve((host, port))
            .await?
            .into_iter()
            .next()
            .ok_or(Error::InvalidHost)?;
        let tcp_s = TcpStream::connect(addr).await?;
        let mut tls_s = async_native_tls::TlsConnector::new()
            .danger_accept_invalid_certs(true) // FIXME: handle certs
            .connect(host, tcp_s)
            .await?;

        let url_request = url.to_string() + "\r\n";
        tls_s.write_all(url_request.as_bytes()).await?;
        debug!("Request sent at {}", url);

        // To save some allocations, the buffer size is pretty big. If the user has a fast internet
        // connection, it may fill the entire buffer with one read syscall. With a slow connection,
        // this buffer will never be filled fully, because the loop below will exit as soon as the
        // end of the meta tag is found, to reduce the streaming latency
        let mut buffer = Vec::with_capacity(INIT_BUFFER_SIZE);
        buffer.extend_from_slice(&[0; INIT_BUFFER_SIZE]);

        let mut n_read = 0;
        let meta_end = loop {
            match tls_s.read(&mut buffer[n_read..]).await {
                Ok(0) => return Err(Error::InvalidProtocolData(ProtoError::MetaNotFound)),
                Ok(n) => {
                    n_read += n;
                    debug!("Received {}", n);
                    // The first three bytes are for status and a space
                    if n_read > 3 {
                        // Find the end of metadata, by looking for <CR><CF> directly by looking
                        // at bytes just received.
                        // Start looking from n_read-n-1 to be sure to include the '\r', which may
                        // have been read before
                        let search_start = (n_read - n).saturating_sub(1); // don't go below 0
                        let meta_end_res = buffer[search_start..n_read]
                            .windows(2)
                            .position(|w| w == b"\r\n");
                        if let Some(i) = meta_end_res {
                            debug!("Found meta at {}", i);
                            break search_start + i;
                        }
                        if n_read > 3 + 1024 {
                            return Err(Error::InvalidProtocolData(ProtoError::MetaNotFound));
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            };
        };

        let status: u8 = std::str::from_utf8(buffer.get(0..2).unwrap_or(&[]))?
            .parse()
            .map_err(|_| Error::InvalidProtocolData(InvalidStatus.into()))?;

        let status = Status::try_from(status)
            .map_err(|_| Error::InvalidProtocolData(InvalidStatus.into()))?;

        let meta_buffer = &buffer.get(3..meta_end).unwrap_or(&[]);
        // Split the part of the buffer containing the meta
        let meta = String::from_utf8_lossy(meta_buffer).to_string();

        buffer.truncate(n_read);
        buffer.shrink_to_fit();

        Ok(Response {
            status,
            // meta_end + 2b offset of status and space + 2b offset (\r\n)
            cursor: Cursor::new(buffer.split_off(meta_end + 2)),
            meta,
            tls_s,
        })
    }
}
#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use crate::gemini::*;
    #[test]
    fn client_builder() {
        let client = ClientBuilder::new().redirect(true).build();

        assert_eq!(client.options, ClientOptions { redirect: true });
    }
    #[test]
    fn home() -> Result<(), Error> {
        block_on(async {
            let client = Client::new();
            let res = client.fetch("gemini://gemini.circumlunar.space/").await?;

            assert_eq!(res.status(), Status::Success(20));
            assert_eq!(res.meta(), "text/gemini");
            assert!(res.body().is_some());

            Ok(())
        })
    }
    #[test]
    fn home_auto_redirect() -> Result<(), Error> {
        block_on(async {
            let client = ClientBuilder::new().redirect(true).build();

            // The url doesn't have a final slash. It's going to be redirected to /
            let res = client.fetch("gemini://gemini.circumlunar.space").await?;

            assert_eq!(res.status(), Status::Success(20));
            assert_eq!(res.meta(), "text/gemini");
            assert!(res.body().is_some());

            Ok(())
        })
    }

    #[test]
    fn home_no_redirect() -> Result<(), Error> {
        block_on(async {
            let client = Client::new();
            // The url doesn't have a final slash. It's going to be redirected to /
            let res = client.fetch("gemini://gemini.circumlunar.space").await?;

            assert_eq!(res.status(), Status::Redirect(31)); // needs redirection
            assert_eq!(res.meta(), "gemini://gemini.circumlunar.space/");
            assert!(res.body().is_none()); // no body in redirections

            Ok(())
        })
    }

    #[test]
    fn invalid_scheme() -> Result<(), Error> {
        block_on(async {
            let client = Client::new();
            // The url doesn't have a final slash. It's going to be redirected to /
            let res = client.fetch("http://gemini.circumlunar.space").await;

            assert!(res.is_err());
            Ok(())
        })
    }
}

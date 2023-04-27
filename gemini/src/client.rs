use std::cell::RefCell;
use std::convert::TryFrom;
use std::rc::Rc;

use futures::io::Cursor;
use futures::prelude::*;
use futures::task::{Context, Poll};
use gio::prelude::*;
use log::debug;
use url::Url;

use crate::{known_hosts, CertificateError};

const MAX_REDIRECT: u8 = 5;
const MAX_TIMEOUT: u32 = 10000;

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
    #[error("Gio error: {0:?}")]
    Gio(String),
    #[error("The server doesn't follow the gemini protocol: {0:?}")]
    InvalidProtocolData(#[from] ProtoError),
    #[error("The server sent invalid utf8: {0:?}")]
    InvalidUtf8(#[from] std::str::Utf8Error),
    #[error("Invalid url: {0:?}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("Tls error: {0:?}")]
    Tls(#[from] CertificateError),
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

pub trait Validator {
    fn validate(&mut self, host: &str, cert: &gio::TlsCertificate) -> Result<(), CertificateError>;
}

impl<F: FnMut(&str, &gio::TlsCertificate) -> Result<(), CertificateError>> Validator for F {
    fn validate(&mut self, host: &str, cert: &gio::TlsCertificate) -> Result<(), CertificateError> {
        self(host, cert)
    }
}

pub struct ConnectionAsyncRead<T: AsyncRead> {
    // WARNING: The connection MUST STAY IN SCOPE while the body is read. If the connection
    // goes out of scope, it gets closed and reading it becomes impossible.
    pub connection: gio::SocketConnection,
    pub readable: T,
}

impl<T: AsyncRead + std::marker::Unpin> AsyncRead for ConnectionAsyncRead<T> {
    // Required method
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let readable = &mut self.as_mut().readable;
        futures::pin_mut!(readable);
        let readable: std::pin::Pin<_> = readable.as_mut();
        AsyncRead::poll_read(readable, cx, buf)
    }
}

pub struct Response {
    status: Status,
    meta: String,
    body: Box<dyn AsyncRead + std::marker::Unpin>,
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
            Status::Success(_) => Some(self.body),
            _ => None,
        }
    }
    pub async fn from_async_read(
        mut async_readable: impl AsyncRead + std::marker::Unpin + 'static,
    ) -> Result<Self, Error> {
        let mut buffer = Vec::with_capacity(2048);
        // 3 bytes for the status, 1024 max bytes for the meta
        (&mut async_readable)
            .take(3 + 1024)
            .read_to_end(&mut buffer)
            .await?;

        let meta_end = buffer[3..]
            .windows(2)
            .position(|w| w == b"\r\n")
            .map(|i| i + 3)
            .ok_or(Error::InvalidProtocolData(ProtoError::MetaNotFound))?;

        let status: u8 = std::str::from_utf8(buffer.get(0..2).unwrap_or(&[]))?
            .parse()
            .map_err(|_| Error::InvalidProtocolData(InvalidStatus.into()))?;

        let status = Status::try_from(status)
            .map_err(|_| Error::InvalidProtocolData(InvalidStatus.into()))?;

        let meta_buffer = &buffer.get(3..meta_end).unwrap_or(&[]);
        // Split the part of the buffer containing the meta
        let meta = String::from_utf8_lossy(meta_buffer).to_string();

        // 2b offset for '\r\n'
        let split_at = meta_end + 2;
        let cursor = Cursor::new(buffer.split_off(split_at));
        Ok(Response {
            status,
            meta,
            body: Box::new(cursor.chain(async_readable)),
        })
    }
}
#[derive(Default, PartialEq, Eq, Debug, Copy, Clone)]
pub struct ClientOptions {
    redirect: bool,
}

#[derive(Default, Clone)]
pub struct ClientBuilder {
    options: ClientOptions,
    validator: Option<Rc<RefCell<dyn Validator>>>,
}

impl std::fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("options", &self.options)
            .field("validator", &self.validator.is_some())
            .finish()
    }
}

impl ClientBuilder {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn redirect(mut self, redirect: bool) -> Self {
        self.options.redirect = redirect;
        self
    }
    pub fn validator(mut self, f: impl Validator + 'static) -> Self {
        self.validator = Some(Rc::new(RefCell::new(f)));
        self
    }
    pub fn build(self) -> Client {
        Client {
            options: self.options,
            validator: self
                .validator
                .unwrap_or_else(|| Client::default_validator()),
        }
    }
}

#[derive(Clone)]
pub struct Client {
    options: ClientOptions,
    validator: Rc<RefCell<dyn Validator>>,
}

impl Default for Client {
    fn default() -> Self {
        Self {
            options: Default::default(),
            validator: Self::default_validator(),
        }
    }
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("options", &self.options)
            .finish()
    }
}

impl Client {
    pub fn default_validator() -> Rc<RefCell<dyn Validator>> {
        let mut known_hosts = known_hosts::KnownHostsMap::new();
        Rc::new(RefCell::new(
            move |host: &str, cert: &gio::TlsCertificate| {
                known_hosts::validate(&mut known_hosts, host, cert)
            },
        ))
    }
    pub fn new() -> Self {
        Self::default()
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
            res = self.fetch_internal(url).await?;
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
    async fn connect(&self, url: Url) -> Result<gio::SocketConnection, Error> {
        if url.scheme() != "gemini" {
            return Err(Error::SchemeNotSupported);
        }
        let host = url.host_str().ok_or(Error::InvalidHost)?.to_owned();
        let addr =
            gio::NetworkAddress::parse_uri(url.as_str(), 1965).map_err(|_| Error::InvalidHost)?;
        let socket = gio::SocketClient::new();
        socket.set_tls(true);
        socket.set_timeout(MAX_TIMEOUT);

        let tls_error = Rc::new(RefCell::new(None));
        let validator = self.validator.clone();
        let tls_error_clone = tls_error.clone();
        socket.connect_event(move |_this, event, _connectable, connection| {
            use gio::SocketClientEvent;
            if event == SocketClientEvent::TlsHandshaking {
                let connection = connection
                    .as_ref()
                    .unwrap()
                    .dynamic_cast_ref::<gio::TlsClientConnection>()
                    .unwrap();

                let host = host.clone();
                let validator = validator.clone();
                let tls_error_clone = tls_error_clone.clone();
                connection.connect_accept_certificate(move |_this, cert, _cert_flags| {
                    match validator.borrow_mut().validate(&host, cert) {
                        Ok(()) => true,
                        Err(e) => {
                            tls_error_clone.replace(Some(e));
                            false
                        }
                    }
                });
            }
        });

        // Open the connection, without checking for errors
        let iostream = socket.connect_future(&addr).await;

        // Handle the custom tls errors, before handling the automatic iostream errors
        if let Some(e) = tls_error.borrow().as_ref() {
            return Err(Error::Tls(*e));
        };

        iostream.map_err(|e| Error::Gio(e.to_string()))
    }
    async fn fetch_internal(&self, url: Url) -> Result<Response, Error> {
        let connection = self.connect(url.clone()).await?;
        let url_request = url.to_string() + "\r\n";
        connection
            .output_stream()
            .write_all_future(url_request.into_bytes(), glib::PRIORITY_DEFAULT)
            .await
            .map_err(|(_, e)| Error::Gio(e.to_string()))?;
        debug!("Request sent at {}", url);

        let readable = connection
            .input_stream()
            .dynamic_cast::<gio::PollableInputStream>()
            .unwrap()
            .into_async_buf_read(1024);

        let async_readable = ConnectionAsyncRead {
            connection,
            readable,
        };
        Response::from_async_read(async_readable).await
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;

    use crate::*;

    fn block_on<T>(f: impl Future<Output = T>) -> T {
        let ctx = glib::MainContext::new();
        let task = f;
        ctx.block_on(task)
    }

    fn response_from_bytes(bytes: &[u8]) -> Result<Response, Error> {
        let async_read = futures::io::Cursor::new(bytes.to_vec());
        block_on(Response::from_async_read(async_read))
    }

    #[test]
    fn client_builder() {
        let client = ClientBuilder::new().redirect(true).build();
        assert_eq!(client.options, ClientOptions { redirect: true });
    }

    #[test]
    fn basic_res() -> Result<(), Error> {
        let res = response_from_bytes(
            b"20 text/gemini\r\n\
            Basic example response from a dummy server",
        )?;
        assert_eq!(res.status(), Status::Success(20));
        Ok(())
    }

    #[test]
    fn unexpected_body() -> Result<(), Error> {
        let res = response_from_bytes(
            b"31 gemini://gemini.circumlunar.space/\r\n\
            Unexpected body",
        )?;
        assert_eq!(res.status(), Status::Redirect(31));
        assert_eq!(res.meta(), "gemini://gemini.circumlunar.space/");
        assert!(res.body().is_none());
        Ok(())
    }

    #[test]
    fn no_meta() -> Result<(), Error> {
        let res = response_from_bytes(
            b"20\r\n\
            Basic example response from a dummy server",
        );
        matches!(
            res,
            Err(Error::InvalidProtocolData(ProtoError::MetaNotFound))
        );
        Ok(())
    }

    #[test]
    fn meta_too_long() -> Result<(), Error> {
        let mut bytes = Vec::from([b' '; 1030]);
        bytes[0] = b'2';
        bytes[1] = b'0';

        // max size of meta is 1024, we go over that
        for i in 2..1030 {
            bytes[i] = b'a';
        }

        let res = response_from_bytes(&bytes.clone());
        matches!(
            res,
            Err(Error::InvalidProtocolData(ProtoError::MetaNotFound))
        );
        Ok(())
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
    fn invalid_scheme() -> Result<(), Error> {
        block_on(async {
            let client = Client::new();
            let res = client.fetch("http://gemini.circumlunar.space").await;
            assert!(res.is_err());
            Ok(())
        })
    }
}

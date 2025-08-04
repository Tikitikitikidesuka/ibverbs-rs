use std::io::{Read, Write};
use ibverbs::QueuePairEndpoint;
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;
use thiserror::Error;

const MAX_MESSAGE_SIZE: usize = 4096;

#[derive(Debug, Error)]
pub enum IbBEndpointExchangeError {
    #[error("Error connecting to Queue pair exchange socket server")]
    ConnectionError(std::io::Error),
    #[error("Queue pair endpoint serialization/deserialization error: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("Queue pair socket IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Message too large: {0} bytes (max: {1})")]
    MessageTooLarge(usize, usize),
    #[error("Invalid UTF-8 in received message")]
    InvalidUtf8(#[from] std::str::Utf8Error),
    #[error("Incomplete message received")]
    IncompleteMessage,
}

pub struct IbBEndpointExchange {
    listener: TcpListener,
}

impl IbBEndpointExchange {
    /// Create a new server that listens on the given address
    pub fn new(socket_addr: impl ToSocketAddrs) -> Result<Self, IbBEndpointExchangeError> {
        let listener = TcpListener::bind(socket_addr)?;
        Ok(Self { listener })
    }

    /// Accept a connection and exchange endpoints with a client
    pub fn accept_and_exchange(
        &self,
        qp_endpoint: QueuePairEndpoint,
        timeout: Duration,
    ) -> Result<QueuePairEndpoint, IbBEndpointExchangeError> {
        let (mut stream, _addr) = self.listener.accept()?;
        Self::exchange_with_stream(&mut stream, qp_endpoint, timeout)
    }

    /// Connect to a server and exchange endpoints (client mode)
    pub fn connect_and_exchange(
        socket_addr: impl ToSocketAddrs,
        qp_endpoint: QueuePairEndpoint,
        timeout: Duration,
    ) -> Result<QueuePairEndpoint, IbBEndpointExchangeError> {
        let mut stream = TcpStream::connect_timeout(
            &socket_addr.to_socket_addrs()?.next().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid address")
            })?,
            timeout,
        )
        .map_err(IbBEndpointExchangeError::ConnectionError)?;

        Self::exchange_with_stream(&mut stream, qp_endpoint, timeout)
    }

    fn exchange_with_stream(
        stream: &mut TcpStream,
        local_endpoint: QueuePairEndpoint,
        timeout: Duration,
    ) -> Result<QueuePairEndpoint, IbBEndpointExchangeError> {
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;

        let json = serde_json::to_string(&local_endpoint)?;
        Self::send_message(stream, &json)?;
        let received_json = Self::receive_message(stream)?;

        Ok(serde_json::from_str(&received_json)?)
    }

    fn send_message(stream: &mut TcpStream, message: &str) -> std::io::Result<()> {
        let bytes = message.as_bytes();
        let len = bytes.len() as u32;
        stream.write_all(&len.to_be_bytes())?;
        stream.write_all(bytes)?;
        stream.flush()?;
        Ok(())
    }

    fn receive_message(stream: &mut TcpStream) -> Result<String, IbBEndpointExchangeError> {
        let mut len_bytes = [0u8; 4];
        stream.read_exact(&mut len_bytes)?;
        let len = u32::from_be_bytes(len_bytes) as usize;

        if len > MAX_MESSAGE_SIZE {
            return Err(IbBEndpointExchangeError::MessageTooLarge(
                len,
                MAX_MESSAGE_SIZE,
            ));
        }

        let mut buffer = vec![0u8; len];
        stream.read_exact(&mut buffer)?;

        Ok(String::from_utf8(buffer)
            .map_err(|e| IbBEndpointExchangeError::InvalidUtf8(e.utf8_error()))?)
    }
}

use ibverbs::QueuePairEndpoint;
use std::io;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;
use thiserror::Error;

const MAX_MESSAGE_SIZE: usize = 1024;

#[derive(Debug, Error)]
pub enum IbBEndpointExchangeError {
    #[error("Error connecting to Queue pair exchange socket server")]
    ConnectionError(io::Error),
    #[error("Queue pair endpoint serialization/deserialization error: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("Queue pair socket IO error: {0}")]
    IoError(#[from] io::Error),
    #[error("Message too large: {0} bytes (max: {1})")]
    MessageTooLarge(usize, usize),
    #[error("Invalid UTF-8 in received message")]
    InvalidUtf8(#[from] std::str::Utf8Error),
    #[error("Incomplete message received")]
    IncompleteMessage,
}

/// Wire protocol: [4-byte length in network byte order][JSON data]
pub struct IbBEndpointExchange;

impl IbBEndpointExchange {
    /// Start a server that exchanges endpoints with a single client
    pub fn exchange_as_server(
        socket_addr: impl ToSocketAddrs,
        qp_endpoint: QueuePairEndpoint,
        timeout: Duration,
    ) -> Result<QueuePairEndpoint, IbBEndpointExchangeError> {
        let listener = TcpListener::bind(socket_addr)?;
        let (mut stream, _addr) = listener.accept()?;

        Self::exchange_with_stream(&mut stream, qp_endpoint, timeout)
    }

    /// Connect to a server and exchange endpoints
    pub fn exchange_as_client(
        socket_addr: impl ToSocketAddrs,
        qp_endpoint: QueuePairEndpoint,
        timeout: Duration,
    ) -> Result<QueuePairEndpoint, IbBEndpointExchangeError> {
        let mut stream = TcpStream::connect_timeout(
            &socket_addr
                .to_socket_addrs()?
                .next()
                .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "Invalid address"))?,
            timeout,
        )
        .map_err(IbBEndpointExchangeError::ConnectionError)?;

        Self::exchange_with_stream(&mut stream, qp_endpoint, timeout)
    }

    /// Perform the actual exchange over an established connection
    fn exchange_with_stream(
        stream: &mut TcpStream,
        local_endpoint: QueuePairEndpoint,
        timeout: Duration,
    ) -> Result<QueuePairEndpoint, IbBEndpointExchangeError> {
        // Set timeouts
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;

        // Serialize our endpoint
        let json = serde_json::to_string(&local_endpoint)?;

        // Send our endpoint
        Self::send_message(stream, &json)?;

        // Receive remote endpoint
        let received_json = Self::receive_message(stream)?;

        // Deserialize and return
        Ok(serde_json::from_str(&received_json)?)
    }

    /// Send a message with length prefix
    fn send_message(stream: &mut TcpStream, message: &str) -> io::Result<()> {
        let bytes = message.as_bytes();
        let len = bytes.len() as u32;

        // Write length prefix (4 bytes, network byte order)
        stream.write_all(&len.to_be_bytes())?;
        // Write message
        stream.write_all(bytes)?;
        stream.flush()?;

        Ok(())
    }

    /// Receive a message with length prefix
    fn receive_message(stream: &mut TcpStream) -> Result<String, IbBEndpointExchangeError> {
        // Read length prefix
        let mut len_bytes = [0u8; 4];
        stream.read_exact(&mut len_bytes)?;
        let len = u32::from_be_bytes(len_bytes) as usize;

        // Validate message size
        if len > MAX_MESSAGE_SIZE {
            return Err(IbBEndpointExchangeError::MessageTooLarge(
                len,
                MAX_MESSAGE_SIZE,
            ));
        }

        // Read message
        let mut buffer = vec![0u8; len];
        stream.read_exact(&mut buffer)?;

        // Convert to string
        Ok(String::from_utf8(buffer)
            .map_err(|e| IbBEndpointExchangeError::InvalidUtf8(e.utf8_error()))?)
    }
}

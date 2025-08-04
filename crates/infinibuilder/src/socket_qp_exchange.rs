use ibverbs::QueuePairEndpoint;
use std::io;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use thiserror::Error;

const EXCHANGE_BUFFER_SIZE: usize = 1024;

#[derive(Debug, Error)]
pub enum IbBEndpointExchangeError {
    #[error("Error connecting to Queue pair exchange socket server")]
    ConnectionError(io::Error),

    #[error("Queue pair endpoint serialization/deserialization error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("Queue pair socket IO error: {0}")]
    IoError(#[from] io::Error),
}

pub struct IbBEndpointExchangeServer;

impl IbBEndpointExchangeServer {
    pub fn exchange_endpoint(
        socket_addr: impl ToSocketAddrs,
        qp_endpoint: QueuePairEndpoint,
    ) -> Result<QueuePairEndpoint, IbBEndpointExchangeError> {
        let listener = TcpListener::bind(socket_addr)?;
        let (mut stream, _socket_addr) = listener.accept()?;

        let qp_endpoint_json = serde_json::to_string(&qp_endpoint)?;

        let mut buffer = [0; EXCHANGE_BUFFER_SIZE];
        let recv_qp_endpoint_json =
            tcp_exchange(&mut stream, qp_endpoint_json.as_str(), &mut buffer)?;

        let qp_endpoint = serde_json::from_str(&recv_qp_endpoint_json)?;

        Ok(qp_endpoint)
    }
}

pub struct IbBEndpointExchangeClient;

impl IbBEndpointExchangeClient {
    pub fn exchange_endpoint(
        socket_addr: impl ToSocketAddrs,
        qp_endpoint: QueuePairEndpoint,
    ) -> Result<QueuePairEndpoint, IbBEndpointExchangeError> {
        let mut stream = TcpStream::connect(socket_addr)
            .map_err(|error| IbBEndpointExchangeError::ConnectionError(error))?;

        let qp_endpoint_json = serde_json::to_string(&qp_endpoint)?;

        let mut buffer = [0; EXCHANGE_BUFFER_SIZE];
        let recv_qp_endpoint_json =
            tcp_exchange(&mut stream, qp_endpoint_json.as_str(), &mut buffer)?;

        let qp_endpoint = serde_json::from_str(&recv_qp_endpoint_json)?;

        Ok(qp_endpoint)
    }
}

fn tcp_exchange<'a>(
    stream: &mut TcpStream,
    message: &str,
    recv_buff: &'a mut [u8],
) -> io::Result<&'a str> {
    // Write
    stream.write_all(message.as_bytes())?;
    stream.flush()?;

    // Read
    let size = stream.read(recv_buff)?;
    let qp_str = str::from_utf8(&recv_buff[..size]).map_err(|e| {
        io::Error::new(
            ErrorKind::InvalidData,
            "Error getting utf8 string from received bytes",
        )
    })?;

    Ok(qp_str)
}

use crate::IbBTcpNodeQpEndpointExchangeError::{
    ConnectionError, InvalidAddress, InvalidUtf8, MessageTooLarge,
};
use crate::{IbBReadyNodeConfig, IbBStaticNodeConfig};
use ibverbs::QueuePairEndpoint;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;
use thiserror::Error;

const MAX_MESSAGE_SIZE: usize = 4096;

#[derive(Debug, Error)]
pub enum IbBTcpNodeQpEndpointExchangeError {
    #[error("Invalid address")]
    InvalidAddress,
    #[error(transparent)]
    ConnectionError(std::io::Error),
    #[error(transparent)]
    SerdeError(#[from] serde_json::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("Serialized data too large: {0} bytes (max: {MAX_MESSAGE_SIZE})")]
    MessageTooLarge(usize),
    #[error(transparent)]
    InvalidUtf8(#[from] std::str::Utf8Error),
    #[error("Incomplete message received")]
    IncompleteMessage,
}

pub struct IbBTcpNodeQpEndpointExchanger {
    listener: TcpListener,
}

impl IbBTcpNodeQpEndpointExchanger {
    /// Create a new server that listens on the given address
    pub fn new(socket_addr: impl ToSocketAddrs) -> Result<Self, IbBTcpNodeQpEndpointExchangeError> {
        let listener = TcpListener::bind(socket_addr)?;
        Ok(Self { listener })
    }

    /// Accept a connection and exchange endpoints with a client
    pub fn accept_and_exchange(
        &self,
        local_node: IbBStaticNodeConfig,
        local_qp_endpoint: QueuePairEndpoint,
        timeout: Duration,
    ) -> Result<IbBReadyNodeConfig, IbBTcpNodeQpEndpointExchangeError> {
        let (mut stream, _addr) = self.listener.accept()?;

        let ready_node = IbBReadyNodeConfig {
            node_config: local_node,
            qp_endpoint: local_qp_endpoint,
        };

        Self::exchange_with_stream(&mut stream, ready_node, timeout)
    }

    /// Connect to a server and exchange endpoints (client mode)
    pub fn connect_and_exchange(
        socket_addr: impl ToSocketAddrs,
        local_node: IbBStaticNodeConfig,
        local_qp_endpoint: QueuePairEndpoint,
        timeout: Duration,
    ) -> Result<IbBReadyNodeConfig, IbBTcpNodeQpEndpointExchangeError> {
        let mut stream = TcpStream::connect_timeout(
            &socket_addr
                .to_socket_addrs()?
                .next()
                .ok_or(InvalidAddress)?,
            timeout,
        )
        .map_err(ConnectionError)?;

        let ready_node = IbBReadyNodeConfig {
            node_config: local_node,
            qp_endpoint: local_qp_endpoint,
        };

        Self::exchange_with_stream(&mut stream, ready_node, timeout)
    }

    fn exchange_with_stream(
        stream: &mut TcpStream,
        local_ready_node: IbBReadyNodeConfig,
        timeout: Duration,
    ) -> Result<IbBReadyNodeConfig, IbBTcpNodeQpEndpointExchangeError> {
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;

        let json = serde_json::to_string(&local_ready_node)?;
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

    fn receive_message(
        stream: &mut TcpStream,
    ) -> Result<String, IbBTcpNodeQpEndpointExchangeError> {
        let mut len_bytes = [0u8; 4];
        stream.read_exact(&mut len_bytes)?;
        let len = u32::from_be_bytes(len_bytes) as usize;

        if len > MAX_MESSAGE_SIZE {
            return Err(MessageTooLarge(len));
        }

        let mut buffer = vec![0u8; len];
        stream.read_exact(&mut buffer)?;

        Ok(String::from_utf8(buffer).map_err(|e| InvalidUtf8(e.utf8_error()))?)
    }
}

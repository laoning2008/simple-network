use crate::acceptor::{StreamAcceptor, TcpAcceptor, UnixAcceptor};
use crate::connector::{StreamConnector, TcpConnector, UnixConnector};

enum StreamType {
    TCP = 1,
    UNIX = 2,
    // VSOCK = 3,
}

pub struct Factory;

impl Factory {
    pub fn create_connector(server_name: &str) -> Box<dyn StreamConnector> {
        match Self::stream_type(server_name) {
            StreamType::TCP => Box::new(TcpConnector::new(server_name)),
            StreamType::UNIX => Box::new(UnixConnector::new(server_name))
        }
    }

    pub async fn create_acceptor(server_name: &str) -> Box<dyn StreamAcceptor> {
        match Self::stream_type(server_name) {
            StreamType::TCP => Box::new(TcpAcceptor::new(server_name.to_string()).await),
            StreamType::UNIX => Box::new(UnixAcceptor::new(server_name.to_string()))
        }
    }

    fn stream_type(server_name: &str) -> StreamType {
        if server_name.contains(":") {
            StreamType::TCP
        } else {
            StreamType::UNIX
        }
    }
}

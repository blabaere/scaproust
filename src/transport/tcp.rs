use std::net;
use std::str;
use super::{ Transport, Connector, Connection };
use mio;
pub struct Tcp;

impl Transport for Tcp {
	fn connector(&self, addr: &str) -> Box<Connector> {
		let socket_addr: net::SocketAddr = str::FromStr::from_str(addr).unwrap();
		Box::new(TcpConnector { addr: socket_addr })
	}
}

struct TcpConnector {
	addr: net::SocketAddr
}

impl Connector for TcpConnector {
	fn connect(&self) -> Box<Connection> {
		let tcp_sock = mio::tcp::TcpSocket::v4().unwrap();
		let (tcp_conn, complete) = tcp_sock.connect(&self.addr).unwrap();	
		Box::new(TcpConnection)
	}
}

struct TcpConnection;

impl Connection for TcpConnection {
	
}
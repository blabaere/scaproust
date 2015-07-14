use std::net;
use std::str;
use std::io;
use super::{ Transport, Connection };
use mio::{ TryRead, TryWrite, Evented };
use mio::tcp;

pub struct Tcp;

impl Transport for Tcp {

	fn connect(&self, addr: &str) -> Result<Box<Connection>, io::Error> {
		match str::FromStr::from_str(addr) {
			Ok(addr) => self.connect(addr),
			Err(_) => Err(io::Error::new(io::ErrorKind::InvalidInput, addr.to_owned()))
		}
	}

}

impl Tcp {

	fn connect(&self, addr: net::SocketAddr) -> Result<Box<Connection>, io::Error> {
		let tcp_stream = try!(tcp::TcpStream::connect(&addr));
		let connection = TcpConnection { stream: tcp_stream };

		Ok(Box::new(connection))
	}
	
}

struct TcpConnection {
	stream: tcp::TcpStream
}

impl Drop for TcpConnection {
	fn drop(&mut self) {
		debug!("dropping some connection");
		self.stream.shutdown(tcp::Shutdown::Both);
	}
}

impl Connection for TcpConnection {
	fn try_read(&mut self, buf: &mut [u8]) -> Result<Option<usize>, io::Error> {
		self.stream.try_read(buf)
	}

	fn try_write(&mut self, buf: &[u8]) -> Result<Option<usize>, io::Error> {
		self.stream.try_write(buf)
	}

	fn as_evented(&self) -> &Evented {
		&self.stream
	}
}
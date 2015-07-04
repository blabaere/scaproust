use std::net;
use std::str;
use std::io;
use super::{ Transport, Connector, Connection };
use mio;
use mio::{ TryRead, TryWrite, Evented };
use mio::tcp;

use EventLoop;

pub struct Tcp;

impl Transport for Tcp {
	fn connector(&self, addr: &str, id: usize) -> Box<Connector> {
		debug!("Tcp::connector {} -> {}", id, addr);
		let socket_addr: net::SocketAddr = str::FromStr::from_str(addr).unwrap();
		Box::new(TcpConnector { addr: socket_addr, id: id })
	}
}

struct TcpConnector {
	addr: net::SocketAddr,
	id: usize
}

impl Connector for TcpConnector {
	fn connect(&self, event_loop: &mut EventLoop) -> Result<Box<Connection>, io::Error> {
		debug!("TcpConnector::connect {} -> {}", self.id, self.addr);
		let tcp_stream = try!(tcp::TcpStream::connect(&self.addr));	
		let token = mio::Token(self.id); 
		let interest = mio::Interest::readable() | mio::Interest::writable() | mio::Interest::hup() | mio::Interest::error();
		let poll_opt = mio::PollOpt::oneshot();
		let connection = TcpConnection { 
			stream: tcp_stream,
			handshake_sent: false,
			handshake_received: false };

		try!(event_loop.register_opt(&connection.stream, token, interest, poll_opt));

		Ok(Box::new(connection))
	}
}

struct TcpConnection {
	stream: tcp::TcpStream,
	handshake_sent: bool,
	handshake_received: bool
}

impl Connection for TcpConnection {
	fn readable(&mut self, event_loop: &mut EventLoop, hint: mio::ReadHint) -> io::Result<()> {
		debug!("TcpConnection::readable");
		if self.handshake_received {
			Ok(())
		} else {
			let mut handshake = [0u8; 8];
			let read = try!(self.stream.try_read(&mut handshake));

			match read {
				Some(0) => {
					debug!("read 0 bytes of handshake !");
				},
				Some(8) => {
					debug!("handshake read !");
					self.handshake_received = true;
				},
				Some(n) => {
					debug!("read {} bytes of handshake !", n);
				},
				None => {
					debug!("handshake NOT read !");
				}
			}

			Ok(())
		}
	}

	fn writable(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
		debug!("TcpConnection::writable");
		if self.handshake_sent {
			let random_stuff = vec!(65, 86, 67, 70, 75, 80, 81, 90);
			let written = try!(self.stream.try_write(&random_stuff));

			match written {
				Some(0) => {
					debug!("written 0 bytes of random !");
				},
				Some(8) => {
					debug!("random written !");
				},
				Some(n) => {
					debug!("written {} bytes of random !", n);
				},
				None => {
					debug!("random NOT written !");
				}
			}
			Ok(())
		} else {
			let handshake = vec!(0, 83, 80, 0, 0, 80, 0, 0);
			let written = try!(self.stream.try_write(&handshake));

			match written {
				Some(0) => {
					debug!("written 0 bytes of handshake !");
				},
				Some(8) => {
					debug!("handshake written !");
					self.handshake_sent = true;
				},
				Some(n) => {
					debug!("written {} bytes of handshake !", n);
				},
				None => {
					debug!("handshake NOT written !");
				}
			}

			/*match self.stream.try_write(&handshake) {
				Ok(None) => Ok(()),
				Ok(Some(_)) => Ok(()),
				Err(e) => Err(e)
			}*/
			Ok(())
		}
	}

	fn as_evented(&self) -> &Evented {
		&self.stream
	}
	/*fn try_write(&mut self, buf: &[u8]) -> io::Result<Option<usize>> {
		self.stream.try_write(buf)
	}
	fn disconnect(&self, event_loop: &mut EventLoop, id: usize) -> io::Result<()> {
		try!(event_loop.deregister(&self.stream));

		Ok(())
	}
	fn reconnect(&self, event_loop: &mut EventLoop, id: usize) -> io::Result<()> {
		debug!("TcpConnection::reconnect {}", id);
		let token = mio::Token(id); 
		let interest = mio::Interest::readable() | mio::Interest::writable() | mio::Interest::hup() | mio::Interest::error();
		let poll_opt = mio::PollOpt::oneshot();

		try!(event_loop.register_opt(&self.stream, token, interest, poll_opt));

		Ok(())
	}*/
}
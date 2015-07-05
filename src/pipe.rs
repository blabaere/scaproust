use std::rc::Rc;
use std::io;

use mio;

use EventLoop;
use protocol::Protocol as Protocol;
use transport::Connection as Connection;

pub struct Pipe {
	id: usize,
	protocol: Rc<Box<Protocol>>,
	connection: Box<Connection>,
	handshake_sent: bool,
	handshake_received: bool,
	msg_sent: bool
}

impl Pipe {

	pub fn new(id: usize, protocol: Rc<Box<Protocol>>, connection: Box<Connection>) -> Pipe {
		Pipe {
			id: id,
			protocol: protocol,
			connection: connection,
			handshake_sent: false,
			handshake_received: false,
			msg_sent: false
		}
	}

	pub fn init(&self, event_loop: &mut EventLoop) -> io::Result<()> {
		let interest = mio::Interest::readable() | mio::Interest::writable() | mio::Interest::hup();
		let poll_opt = mio::PollOpt::oneshot();

		try!(self.register(event_loop, interest, poll_opt));

		Ok(())
	}

	pub fn readable(&mut self, event_loop: &mut EventLoop, hint: mio::ReadHint)	-> io::Result<()> {
		debug!("pipe [{}] readable: {:?}", self.id, hint);
		if self.handshake_received {
			debug!("pipe [{}] push need no read: {:?}", self.id, hint);
			let mut packet = [0u8; 1024];
			let read = try!(self.connection.try_read(&mut packet));

			match read {
				Some(0) => {
					debug!("read 0 bytes of msg !");
				},
				Some(n) => {
					debug!("read {} bytes of msg !", n);
				},
				None => {
					debug!("read NOTHING !");
				}
			}
			Ok(())
		} else {
			let mut handshake = [0u8; 8];
			let read = try!(self.connection.try_read(&mut handshake));

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

			let mut interest = mio::Interest::hup();
			let poll_opt = mio::PollOpt::oneshot();

			if self.handshake_received == false {
				interest = interest | mio::Interest::readable();
			} 

			if self.handshake_sent == false {
				interest = interest | mio::Interest::writable();
			} 

			if self.msg_sent == false {
				interest = interest | mio::Interest::writable();
			} 
			
			try!(self.reregister(event_loop, interest, poll_opt));

			Ok(())			
		}
	}

	pub fn writable(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
		if self.handshake_sent {
			if self.handshake_received {
				let header = [0, 0, 0, 0, 0, 0, 0, 3];
				let body = [65, 86, 67];

				try!(self.connection.try_write(&header));
				debug!("packet header sent !");
				try!(self.connection.try_write(&body));
				debug!("packet body sent !");
				self.msg_sent = true;
				/*let random_stuff = vec!(0, 0, 0, 0, 0, 0, 0, 8, 65, 86, 67, 70, 75, 80, 81, 90);
				let written = try!(self.stream.try_write(&random_stuff));

				try!(self.stream.try_write(&header));
				try!(self.stream.try_write(&random_stuff));

				match written {
					Some(0) => {
						debug!("written 0 bytes of random !");
					},
					Some(n) => {
						debug!("written {} bytes of random !", n);
					},
					None => {
						debug!("random NOT written !");
					}
				}*/
			} else {
				debug!("writable but waiting for peer handshake !");
			}
		} else {
			let handshake = vec!(0, 83, 80, 0, 0, 80, 0, 0);
			let written = try!(self.connection.try_write(&handshake));

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
		}

		if self.msg_sent {
			// handshake and first dummy msg sent, just see if we become writable again
			let interest = mio::Interest::writable() | mio::Interest::hup();
			let poll_opt = mio::PollOpt::edge();
			try!(self.reregister(event_loop, interest, poll_opt));
		} else {
			if self.handshake_sent {
				// handshake sent, but not dummy msg
				if self.handshake_received {
					// handshake sent & received, but not dummy msg
					let interest = mio::Interest::writable() | mio::Interest::hup();
					let poll_opt = mio::PollOpt::oneshot();
					try!(self.reregister(event_loop, interest, poll_opt));
				} else {
					// handshake sent but not received
					let interest = mio::Interest::readable() | mio::Interest::hup();
					let poll_opt = mio::PollOpt::oneshot();
					try!(self.reregister(event_loop, interest, poll_opt));
				}
			} else {
				// writable but handshake not send : problem with peer
				// if the write was blocked => retry, otherwise => disconnect and schedule reconnect
			}
		}

		Ok(())
	}

	fn register(&self, event_loop: &mut EventLoop, interest: mio::Interest, poll_opt: mio::PollOpt) -> io::Result<()> {
		let token = mio::Token(self.id); 
		let fd = self.connection.as_evented();

		event_loop.register_opt(fd, token, interest, poll_opt)
	}

	fn reregister(&self, event_loop: &mut EventLoop, interest: mio::Interest, poll_opt: mio::PollOpt) -> io::Result<()> {
		let token = mio::Token(self.id); 
		let fd = self.connection.as_evented();

		event_loop.reregister(fd, token, interest, poll_opt)
	}
	
}
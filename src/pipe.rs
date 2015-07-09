use std::rc::Rc;
use std::io;

use byteorder::{BigEndian, WriteBytesExt};

use mio;

use EventLoop;
use protocol::Protocol as Protocol;
use transport::Connection as Connection;

enum PipeStateIdx {
	Handshake,
	Ready
}

// TODO : looks like the only option is to:
// - keep all states inside a collection field (use Slab ?)
// - keep track of the current state inside an index field (use Token ?)
// - have the state changing occur by passing around a new index value of the position field ...
// - use constants to identify each state
pub struct Pipe {
	state: PipeStateIdx,
	handshake_state: HandshakePipeState
}

impl Pipe {

	pub fn new(id: usize, protocol: Rc<Box<Protocol>>, connection: Box<Connection>) -> Pipe {
		Pipe {
			state: PipeStateIdx::Handshake,
			handshake_state: HandshakePipeState::new(id, protocol, connection)
		}
	}

	fn get_state<'a>(&'a mut self) -> &'a mut PipeState {
		match self.state {
			PipeStateIdx::Handshake => &mut self.handshake_state,
			PipeStateIdx::Ready => &mut self.handshake_state // todo implement the ready state
		}
	}

	fn set_state(&mut self, state: PipeStateIdx, event_loop: &mut EventLoop) -> io::Result<()> {
		self.state = state;
		self.get_state().enter(event_loop)
	}

	pub fn init(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
		self.set_state(PipeStateIdx::Handshake, event_loop)
		//self.state.take().unwrap().enter(event_loop)
		/*let interest = mio::Interest::readable() | mio::Interest::writable() | mio::Interest::hup();
		let poll_opt = mio::PollOpt::oneshot();

		try!(self.register(event_loop, interest, poll_opt));

		Ok(())*/
	}

	pub fn readable(&mut self, event_loop: &mut EventLoop, hint: mio::ReadHint)	-> io::Result<()> {
		//let state = ;
		if let Some(new_state) = try!(self.get_state().readable(event_loop)) {
			self.set_state(new_state, event_loop);
		} 

		Ok(())
		/*if self.msg_sent {
			return Ok(())
		}

		let next = try!(self.state.readable(&mut self.connection, event_loop));
		Ok(())*/
		/*debug!("pipe [{}] readable: {:?}", self.id, hint);
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
					//TODO validate peer protocol number !
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
		}*/
	}

	pub fn writable(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
		/*let mut state = self.state.take().unwrap();

		if try!(state.writable(event_loop)) {
			self.state = Some(state.leave());
		} else {
			self.state = Some(state);
		}*/

		Ok(())
		//let state = &self.state;
		//let next = state.writable(self, event_loop);
		//Ok(())
		/*if self.handshake_sent {
			if self.handshake_received {
				if self.msg_sent {
					debug!("writable but nothing to do anymore !");
				} else {
					let header = [0, 0, 0, 0, 0, 0, 0, 3];
					let body = [65, 86, 67];

					try!(self.connection.try_write(&header));
					try!(self.connection.try_write(&body));
					self.msg_sent = true;
				}
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
			// handshake is Zero, 'S', 'P', Version, Proto, Rsvd
			let mut handshake = vec!(0, 83, 80, 0);
			handshake.write_u16::<BigEndian>(self.protocol.id()).unwrap();
			handshake.write_u16::<BigEndian>(0).unwrap();
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

		Ok(())*/
	}
	
}

trait PipeState {
	fn enter(&mut self, event_loop: &mut EventLoop) -> io::Result<()>;
	fn readable(&mut self, event_loop: &mut EventLoop) -> io::Result<Option<PipeStateIdx>>;
	fn writable(&mut self, event_loop: &mut EventLoop) -> io::Result<Option<PipeStateIdx>>;

	/*fn register(&self, event_loop: &mut EventLoop, interest: mio::Interest, poll_opt: mio::PollOpt) -> io::Result<()> {
		let token = mio::Token(self.id); 
		let fd = self.connection.as_evented();

		event_loop.register_opt(fd, token, interest, poll_opt)
	}

	fn reregister(&self, event_loop: &mut EventLoop, interest: mio::Interest, poll_opt: mio::PollOpt) -> io::Result<()> {
		let token = mio::Token(self.id); 
		let fd = self.connection.as_evented();

		event_loop.reregister(fd, token, interest, poll_opt)
	}*/
}

struct HandshakePipeState {
	id: usize,
	protocol: Rc<Box<Protocol>>,
	connection: Box<Connection>,
	sent: bool,
	received: bool
}

impl HandshakePipeState {
	fn new(id: usize, protocol: Rc<Box<Protocol>>, connection: Box<Connection>) -> HandshakePipeState {
		HandshakePipeState { 
			id: id,
			protocol: protocol,
			connection: connection,
			sent: false, 
			received: false }
	}

	fn check_received_handshake(&self/*, pipe: &Pipe*/, handshake: &[u8; 8]) -> io::Result<()> {
		let mut expected_handshake = vec!(0, 83, 80, 0);
		let protocol_id = 81u16;//pipe.protocol.peer_id();
		try!(expected_handshake.write_u16::<BigEndian>(protocol_id));
		try!(expected_handshake.write_u16::<BigEndian>(0));
		let mut both = handshake.iter().zip(expected_handshake.iter());

		if both.all(|(l,r)| l == r) {
			Ok(())
		} else {
			Err(io::Error::new(io::ErrorKind::InvalidData, "received wrong handshake"))
		}
	}

	fn check_sent_handshake(&self, written: Option<usize>) -> io::Result<()> {
		match written {
			Some(8) => Ok(()),
			_ => Err(io::Error::new(io::ErrorKind::InvalidData, "failed to send handshake"))
		}
	}
}

impl PipeState for HandshakePipeState {
	fn enter(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
		let interest = mio::Interest::readable() | mio::Interest::writable() | mio::Interest::hup();
		let poll_opt = mio::PollOpt::oneshot();

		//try!(self.register(event_loop, interest, poll_opt));

		Ok(())
	}

	fn readable(&mut self, event_loop: &mut EventLoop) -> io::Result<Option<PipeStateIdx>> {
		if self.received {
			return Ok(None)
		}

		let mut handshake = [0u8; 8];
		try!(
			self.connection.try_read(&mut handshake).
			and_then(|_| self.check_received_handshake(&handshake)));

		debug!("handshake received !");
		self.received = true;

		if self.sent {
			Ok(Some(PipeStateIdx::Ready))			
		} else {
			/*let interest = mio::Interest::hup() | mio::Interest::writable();
			let poll_opt = mio::PollOpt::oneshot();

			try!(pipe.reregister(event_loop, interest, poll_opt));*/
			Ok(None)			
		}
	}

	fn writable(&mut self, event_loop: &mut EventLoop) -> io::Result<Option<PipeStateIdx>> {
		if self.sent {
			return Ok(None)
		}
		// handshake is Zero, 'S', 'P', Version, Proto, Rsvd
		let mut handshake = vec!(0, 83, 80, 0);
		let protocol_id = self.protocol.id();
		try!(handshake.write_u16::<BigEndian>(protocol_id));
		try!(handshake.write_u16::<BigEndian>(0));

		try!(
			self.connection.try_write(&handshake).
			and_then(|w| self.check_sent_handshake(w)));

		debug!("handshake received !");
		self.sent = true;

		if self.received {
			Ok(Some(PipeStateIdx::Ready))
		} else {
			//let interest = mio::Interest::hup() | mio::Interest::readable();
			//let poll_opt = mio::PollOpt::oneshot();
			//try!(pipe.reregister(event_loop, interest, poll_opt));
			Ok(None)
		}
	}
}
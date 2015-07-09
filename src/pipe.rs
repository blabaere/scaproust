use std::rc::Rc;
use std::cell::RefCell;
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

pub struct Pipe {
	state: PipeStateIdx,
	handshake_state: HandshakePipeState,
	ready_state: ReadyPipeState
}

impl Pipe {

	pub fn new(id: usize, protocol: Rc<Box<Protocol>>, connection: Box<Connection>) -> Pipe {
		let conn_ref = Rc::new(RefCell::new(connection));

		Pipe {
			state: PipeStateIdx::Handshake,
			handshake_state: HandshakePipeState::new(id, protocol.clone(), conn_ref.clone()),
			ready_state: ReadyPipeState::new(id, protocol.clone(), conn_ref.clone())
		}
	}

	fn get_state<'a>(&'a mut self) -> &'a mut PipeState {
		match self.state {
			PipeStateIdx::Handshake => &mut self.handshake_state,
			PipeStateIdx::Ready => &mut self.ready_state
		}
	}

	fn set_state(&mut self, state: PipeStateIdx, event_loop: &mut EventLoop) -> io::Result<()> {
		self.state = state;
		self.get_state().enter(event_loop)
	}

	pub fn init(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
		self.set_state(PipeStateIdx::Handshake, event_loop)
	}

	pub fn readable(&mut self, event_loop: &mut EventLoop, hint: mio::ReadHint)	-> io::Result<()> {
		if let Some(new_state) = try!(self.get_state().readable(event_loop)) {
			try!(self.set_state(new_state, event_loop));
		} 

		Ok(())
	}

	pub fn writable(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
		if let Some(new_state) = try!(self.get_state().writable(event_loop)) {
			try!(self.set_state(new_state, event_loop));
		} 

		Ok(())
	}
	
}

trait PipeState {
	fn enter(&mut self, event_loop: &mut EventLoop) -> io::Result<()>;
	fn readable(&mut self, event_loop: &mut EventLoop) -> io::Result<Option<PipeStateIdx>>;
	fn writable(&mut self, event_loop: &mut EventLoop) -> io::Result<Option<PipeStateIdx>>;
}

struct HandshakePipeState {
	id: usize,
	protocol: Rc<Box<Protocol>>,
	connection: Rc<RefCell<Box<Connection>>>,
	sent: bool,
	received: bool
}

impl HandshakePipeState {

	fn new(id: usize, protocol: Rc<Box<Protocol>>, connection: Rc<RefCell<Box<Connection>>>) -> HandshakePipeState {
		HandshakePipeState { 
			id: id,
			protocol: protocol,
			connection: connection,
			sent: false, 
			received: false }
	}

	fn check_received_handshake(&self, handshake: &[u8; 8]) -> io::Result<()> {
		let mut expected_handshake = vec!(0, 83, 80, 0);
		let protocol_id = self.protocol.peer_id();
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

	fn read_handshake(&mut self) -> io::Result<()> {
		let mut handshake = [0u8; 8];
		let mut connection = self.connection.borrow_mut();
		try!(
			connection.try_read(&mut handshake).
			and_then(|_| self.check_received_handshake(&handshake)));
		debug!("handshake received !");
		self.received = true;
		Ok(())
	}

	fn write_handshake(&mut self) -> io::Result<()> {
		// handshake is Zero, 'S', 'P', Version, Proto, Rsvd
		let mut handshake = vec!(0, 83, 80, 0);
		let protocol_id = self.protocol.id();
		try!(handshake.write_u16::<BigEndian>(protocol_id));
		try!(handshake.write_u16::<BigEndian>(0));
		let mut connection = self.connection.borrow_mut();
		try!(
			connection.try_write(&handshake).
			and_then(|w| self.check_sent_handshake(w)));
		debug!("handshake sent !");
		self.sent = true;
		Ok(())
	}

	fn register(&self, event_loop: &mut EventLoop, interest: mio::Interest, poll_opt: mio::PollOpt) -> io::Result<()> {
		let token = mio::Token(self.id); 
		let connection = self.connection.borrow();
		let fd = connection.as_evented();

		event_loop.register_opt(fd, token, interest, poll_opt)
	}

	fn reregister(&self, event_loop: &mut EventLoop, interest: mio::Interest, poll_opt: mio::PollOpt) -> io::Result<()> {
		let token = mio::Token(self.id); 
		let connection = self.connection.borrow();
		let fd = connection.as_evented();

		event_loop.reregister(fd, token, interest, poll_opt)
	}

}

impl PipeState for HandshakePipeState {

	fn enter(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
		debug!("Enter handshake state");
		self.sent = false;
		self.received = false;

		let interest = mio::Interest::readable() | mio::Interest::writable() | mio::Interest::hup();
		let poll_opt = mio::PollOpt::oneshot();

		try!(self.register(event_loop, interest, poll_opt));

		Ok(())
	}

	fn readable(&mut self, event_loop: &mut EventLoop) -> io::Result<Option<PipeStateIdx>> {
		if self.received {
			return Ok(None)
		}

		try!(self.read_handshake());

		if self.sent {
			Ok(Some(PipeStateIdx::Ready))			
		} else {
			let interest = mio::Interest::hup() | mio::Interest::writable();
			let poll_opt = mio::PollOpt::oneshot();

			try!(self.reregister(event_loop, interest, poll_opt));
			Ok(None)			
		}
	}

	fn writable(&mut self, event_loop: &mut EventLoop) -> io::Result<Option<PipeStateIdx>> {
		if self.sent {
			return Ok(None)
		}

		try!(self.write_handshake());

		if self.received {
			Ok(Some(PipeStateIdx::Ready))
		} else {
			let interest = mio::Interest::hup() | mio::Interest::readable();
			let poll_opt = mio::PollOpt::oneshot();
			try!(self.reregister(event_loop, interest, poll_opt));
			Ok(None)
		}
	}
	
}

struct ReadyPipeState {
	id: usize,
	protocol: Rc<Box<Protocol>>,
	connection: Rc<RefCell<Box<Connection>>>,
	sent: bool
}

impl ReadyPipeState {
	fn new(id: usize, protocol: Rc<Box<Protocol>>, connection: Rc<RefCell<Box<Connection>>>) -> ReadyPipeState {
		ReadyPipeState { 
			id: id,
			protocol: protocol,
			connection: connection,
			sent: false }
	}	
}

impl PipeState for ReadyPipeState {
	fn enter(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
		debug!("Enter ready state");
		let token = mio::Token(self.id); 
		let connection = self.connection.borrow_mut();
		let fd = connection.as_evented();
		let interest = mio::Interest::hup() | mio::Interest::writable();
		let poll_opt = mio::PollOpt::edge();
		try!(event_loop.reregister(fd, token, interest, poll_opt));
		Ok(())
	}

	fn readable(&mut self, _: &mut EventLoop) -> io::Result<Option<PipeStateIdx>> {
		Ok(None)
	}

	fn writable(&mut self, _: &mut EventLoop) -> io::Result<Option<PipeStateIdx>> {
		if self.sent {
			Ok(None)
		} else {
			let mut connection = self.connection.borrow_mut();
			let header = [0, 0, 0, 0, 0, 0, 0, 3];
			let body = [65, 86, 67];

			try!(connection.try_write(&header));
			try!(connection.try_write(&body));
			self.sent = true;
			debug!("dummy msg sent !");

			Ok(None)
		}
	}
}
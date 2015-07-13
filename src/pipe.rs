use std::rc::Rc;
use std::cell::RefCell;
use std::io;

use byteorder::{BigEndian, WriteBytesExt};

use mio;

use EventLoop;
use Message;
use protocol::Protocol as Protocol;
use transport::Connection as Connection;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PipeStateIdx {
	Handshake,
	Connected
}

pub struct Pipe {
	state: PipeStateIdx,
	handshake_state: HandshakePipeState,
	connected_state: ConnectedPipeState
}

impl Pipe {

	pub fn new(id: usize, protocol: &Protocol, connection: Box<Connection>) -> Pipe {
		let conn_ref = Rc::new(RefCell::new(connection));

		Pipe {
			state: PipeStateIdx::Handshake,
			handshake_state: HandshakePipeState::new(id, protocol, conn_ref.clone()),
			connected_state: ConnectedPipeState::new(id, conn_ref.clone())
		}
	}

	pub fn is_ready(&self) -> bool {
		self.state == PipeStateIdx::Connected
	}

	pub fn send(&mut self, msg: Message) {
		self.get_state().send(msg);
	}

	fn get_state<'a>(&'a mut self) -> &'a mut PipeState {
		match self.state {
			PipeStateIdx::Handshake => &mut self.handshake_state,
			PipeStateIdx::Connected => &mut self.connected_state
		}
	}

	fn set_state(&mut self, state: PipeStateIdx, event_loop: &mut EventLoop) -> io::Result<()> {
		self.state = state;
		self.get_state().enter(event_loop)
	}

	pub fn init(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
		self.set_state(PipeStateIdx::Handshake, event_loop)
	}

	pub fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet)-> io::Result<()> {
		if events.is_readable() {
			try!(self.readable(event_loop));
		} 
		if events.is_writable() {
			try!(self.writable(event_loop));
		} 
		Ok(())
	}

	pub fn readable(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
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

	fn send(&mut self, msg: Message) -> io::Result<()>;
}

struct HandshakePipeState {
	id: usize,
	protocol_id: u16,
	protocol_peer_id: u16,
	connection: Rc<RefCell<Box<Connection>>>,
	sent: bool,
	received: bool
}

impl HandshakePipeState {

	fn new(id: usize, protocol: &Protocol, connection: Rc<RefCell<Box<Connection>>>) -> HandshakePipeState {
		HandshakePipeState { 
			id: id,
			protocol_id: protocol.id(),
			protocol_peer_id: protocol.peer_id(),
			connection: connection,
			sent: false, 
			received: false }
	}

	fn check_received_handshake(&self, handshake: &[u8; 8]) -> io::Result<()> {
		let mut expected_handshake = vec!(0, 83, 80, 0);
		try!(expected_handshake.write_u16::<BigEndian>(self.protocol_peer_id));
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
		try!(handshake.write_u16::<BigEndian>(self.protocol_id));
		try!(handshake.write_u16::<BigEndian>(0));
		let mut connection = self.connection.borrow_mut();
		try!(
			connection.try_write(&handshake).
			and_then(|w| self.check_sent_handshake(w)));
		debug!("handshake sent !");
		self.sent = true;
		Ok(())
	}

	fn register(&self, event_loop: &mut EventLoop, interest: mio::EventSet, poll_opt: mio::PollOpt) -> io::Result<()> {
		let token = mio::Token(self.id); 
		let connection = self.connection.borrow();
		let fd = connection.as_evented();

		event_loop.register_opt(fd, token, interest, poll_opt)
	}

	fn reregister(&self, event_loop: &mut EventLoop, interest: mio::EventSet, poll_opt: mio::PollOpt) -> io::Result<()> {
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

		let interest = mio::EventSet::readable() | mio::EventSet::writable() | mio::EventSet::hup();
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
			Ok(Some(PipeStateIdx::Connected))			
		} else {
			let interest = mio::EventSet::hup() | mio::EventSet::writable();
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
			Ok(Some(PipeStateIdx::Connected))
		} else {
			let interest = mio::EventSet::hup() | mio::EventSet::readable();
			let poll_opt = mio::PollOpt::oneshot();
			try!(self.reregister(event_loop, interest, poll_opt));
			Ok(None)
		}
	}

	fn send(&mut self, _: Message) -> io::Result<()> {
		Ok(())
	}
	
}

struct ConnectedPipeState {
	id: usize,
	connection: Rc<RefCell<Box<Connection>>>
}

impl ConnectedPipeState {
	fn new(id: usize, connection: Rc<RefCell<Box<Connection>>>) -> ConnectedPipeState {
		ConnectedPipeState { 
			id: id,
			connection: connection
		}
	}	
}

impl PipeState for ConnectedPipeState {
	fn enter(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
		debug!("Enter ready state");
		let token = mio::Token(self.id); 
		let connection = self.connection.borrow_mut();
		let fd = connection.as_evented();
		let interest = mio::EventSet::hup() | mio::EventSet::writable();
		let poll_opt = mio::PollOpt::edge();
		try!(event_loop.reregister(fd, token, interest, poll_opt));
		Ok(())
	}

	fn readable(&mut self, _: &mut EventLoop) -> io::Result<Option<PipeStateIdx>> {
		// take care of pending read here
		Ok(None)
	}

	fn writable(&mut self, _: &mut EventLoop) -> io::Result<Option<PipeStateIdx>> {
		//take care of pending write here
		Ok(None)
		/*if self.sent {
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
		}*/
	}

	fn send(&mut self, msg: Message) -> io::Result<()> {
		let mut connection = self.connection.borrow_mut();
		let mut prefix = Vec::with_capacity(8);
		try!(prefix.write_u64::<BigEndian>(msg.len() as u64));

		try!(connection.try_write(&prefix));

		if msg.get_header().len() > 0 {
			try!(connection.try_write(msg.get_header()));
		}

		if msg.get_body().len() > 0 {
			try!(connection.try_write(msg.get_body()));
		}

		debug!("msg sent !");
		Ok(())
	}
}

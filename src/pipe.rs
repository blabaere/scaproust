use std::rc::Rc;
use std::cell::RefCell;
use std::io;

use byteorder::{BigEndian, WriteBytesExt};

use mio;

use EventLoop;
use Message;
use protocol::Protocol as Protocol;
use transport::Connection as Connection;

pub enum SendStatus {
    Rejected(Message),   // Message can't be sent at the moment
    Completed,       // Message has been successfully sent
    Started          // Message has been partially sent, will finish later
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum State {
	Handshake,
	Connected
}

// A pipe is responsible for handshaking with its peer and transfering messages over a connection.
// That means adding the size prefix to message bytes transfering the bytes
// according to the connection readiness and the operation progress
pub struct Pipe {
	addr: String,
	state: State,
	handshake_state: HandshakePipeState,
	connected_state: ConnectedPipeState
}

impl Pipe {

	pub fn new(id: usize, addr: String, protocol: &Protocol, connection: Box<Connection>) -> Pipe {
		let conn_ref = Rc::new(RefCell::new(connection));

		Pipe {
			addr: addr,
			state: State::Handshake,
			handshake_state: HandshakePipeState::new(id, protocol, conn_ref.clone()),
			connected_state: ConnectedPipeState::new(id, conn_ref.clone())
		}
	}

	// result can be :
	//  - can't send (write prefix would block), transfer back msg ownership
	//  - message successfully sent
	//  - message partially sent, will finish later 
	//  - tried to send, but an error occured while sending or write prefix partial write, msg is lost

	pub fn send(&mut self, event_loop: &mut EventLoop, msg: Message) -> io::Result<SendStatus> {
		self.get_state().send(event_loop, msg)
	}

	pub fn addr(self) -> String {
		self.addr
	}

	fn get_state<'a>(&'a mut self) -> &'a mut PipeState {
		match self.state {
			State::Handshake => &mut self.handshake_state,
			State::Connected => &mut self.connected_state
		}
	}

	fn set_state(&mut self, state: State, event_loop: &mut EventLoop) -> io::Result<()> {
		self.state = state;
		self.get_state().enter(event_loop)
	}

	pub fn init(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
		self.set_state(State::Handshake, event_loop)
	}

	pub fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet)-> io::Result<()> {
		if events.is_hup() {
			return Err(io::Error::new(io::ErrorKind::ConnectionReset, "hup"));
		}

		if events.is_error() {
			return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "hup"));
		}

		if let Some(new_state) = try!(self.get_state().ready(event_loop, events)) {
			return self.set_state(new_state, event_loop);
		} 

		Ok(())
	}
	
}

trait PipeState {
	fn enter(&mut self, event_loop: &mut EventLoop) -> io::Result<()>;
	fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet)-> io::Result<Option<State>>;

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message) -> io::Result<SendStatus>;
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
			_ => Err(io::Error::new(io::ErrorKind::Other, "failed to send handshake"))
		}
	}

	fn read_handshake(&mut self) -> io::Result<()> {
		let mut handshake = [0u8; 8];
		let mut connection = self.connection.borrow_mut();
		try!(
			connection.try_read(&mut handshake).
			and_then(|_| self.check_received_handshake(&handshake)));
		debug!("[{}] handshake received.", self.id);
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
		debug!("[{}] handshake sent.", self.id);
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
		debug!("[{}] enter handshake state", self.id);
		self.sent = false;
		self.received = false;

		let interest = mio::EventSet::readable() | mio::EventSet::writable() | mio::EventSet::hup();
		let poll_opt = mio::PollOpt::oneshot();

		try!(self.register(event_loop, interest, poll_opt));

		Ok(())
	}

	fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet)-> io::Result<Option<State>> {
		if !self.received && events.is_readable() {
			try!(self.read_handshake());
		}

		if !self.sent && events.is_writable() {
			try!(self.write_handshake());
		}

		if self.received && self.sent {
			return Ok(Some(State::Connected));
		}

		let mut interest = mio::EventSet::hup();
		let poll_opt = mio::PollOpt::oneshot();

		if !self.received {
			interest = interest | mio::EventSet::readable();
		}
		if !self.sent {
			interest = interest | mio::EventSet::writable();
		}

		try!(self.reregister(event_loop, interest, poll_opt));

		Ok(None)
	}

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message) -> io::Result<SendStatus> {
		Ok(SendStatus::Rejected(msg))
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
		debug!("[{}] enter connected state", self.id);
		let token = mio::Token(self.id); 
		let connection = self.connection.borrow_mut();
		let fd = connection.as_evented();
		let interest = mio::EventSet::hup() | mio::EventSet::error();
		let poll_opt = mio::PollOpt::edge();
		try!(event_loop.reregister(fd, token, interest, poll_opt));
		Ok(())
	}

	fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet)-> io::Result<Option<State>> {
		Ok(None)
	}

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message) -> io::Result<SendStatus> {
		Ok(SendStatus::Rejected(msg))
	}

	/*fn send(&mut self, msg: Message) -> io::Result<()> {
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
	}*/
}

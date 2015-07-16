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
enum State {
	Handshake,
	Connected
}

// A pipe is responsible for handshaking with its peer and transfering messages over a connection.
// That means send/receive size prefix and then message payload
// according to the connection readiness and the operation progress
pub struct Pipe {
	addr: String,
	state: State,
	handshake_state: HandshakePipeState,
	connected_state: ConnectedPipeState
}

pub enum SendStatus {
	Postponed(Rc<Message>), // Message can't be sent at the moment : Handshake in progress or would block
    Completed,              // Message has been successfully sent
    InProgress              // Message has been partially sent, will finish later
}

impl Pipe {

	pub fn new(id: usize, addr: String, protocol: &Protocol, connection: Box<Connection>) -> Pipe {
		let conn_ref = Rc::new(RefCell::new(connection));

		// TODO : maybe the connection could be stored inside the Pipe 
		// and then passed as &mut to the state methods ?

		Pipe {
			addr: addr,
			state: State::Handshake,
			handshake_state: HandshakePipeState::new(id, protocol, conn_ref.clone()),
			connected_state: ConnectedPipeState::new(id, conn_ref.clone())
		}
	}

	pub fn send(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>) -> io::Result<SendStatus> {
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
			return Err(io::Error::new(io::ErrorKind::ConnectionReset, "event: hup"));
		}

		if events.is_error() {
			return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "event: error"));
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

	fn send(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>) -> io::Result<SendStatus>;
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
			Err(io::Error::new(io::ErrorKind::InvalidData, "received bad handshake"))
		}
	}

	fn check_sent_handshake(&self, written: Option<usize>) -> io::Result<()> {
		match written {
			Some(8) => Ok(()),
			Some(_) => Err(io::Error::new(io::ErrorKind::WouldBlock, "failed to send full handshake")),
			_       => Err(io::Error::new(io::ErrorKind::WouldBlock, "failed to send handshake"))
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

	fn send(&mut self, _: &mut EventLoop, msg: Rc<Message>) -> io::Result<SendStatus> {
		Ok(SendStatus::Postponed(msg))
	}
}

struct ConnectedPipeState {
	id: usize,
	connection: Rc<RefCell<Box<Connection>>>,
	pending_send: Option<Rc<Message>>,
	pending_step: u8,        // 1=prefix, 2=header, 3=body
	pending_progress: usize  // bytes written
	// TODO move these fields inside a SendOperation struct
	// and change pending_send: Option<SendOperation>
}

impl ConnectedPipeState {

	fn new(id: usize, connection: Rc<RefCell<Box<Connection>>>) -> ConnectedPipeState {
		ConnectedPipeState { 
			id: id,
			connection: connection,
			pending_send: None,
			pending_step: 0,
			pending_progress: 0
		}
	}	

}

impl PipeState for ConnectedPipeState {
	fn enter(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
		debug!("[{}] enter connected state", self.id);
		let token = mio::Token(self.id); 
		let connection = self.connection.borrow_mut();
		let fd = connection.as_evented();
		let interest = mio::EventSet::all();
		let poll_opt = mio::PollOpt::edge();
		try!(event_loop.reregister(fd, token, interest, poll_opt));
		Ok(())
	}

	fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet)-> io::Result<Option<State>> {
		// TODO : check if there is a message pending to be sent
		Ok(None)
	}

	fn send(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>) -> io::Result<SendStatus> {
		// TODO just create a SendOperation and let it work from the begining ?
		let mut connection = self.connection.borrow_mut();
		let mut prefix = Vec::with_capacity(8);
		try!(prefix.write_u64::<BigEndian>(msg.len() as u64));

		// try send prefix
		match try!(connection.try_write(&prefix)) {
			Some(x) => if x < 8 {
				self.pending_send = Some(msg);
				self.pending_step = 1;
				self.pending_progress = x;
				return Ok(SendStatus::InProgress);
			},
			None => {
				return Ok(SendStatus::Postponed(msg));
			}
		}

		// try send msg header
		match try!(connection.try_write(msg.get_header())) {
			Some(x) => if x < msg.get_header().len() {
				self.pending_send = Some(msg);
				self.pending_step = 2;
				self.pending_progress = x;
				return Ok(SendStatus::InProgress);
			},
			None => {
				self.pending_send = Some(msg);
				self.pending_step = 2;
				self.pending_progress = 0;
				return Ok(SendStatus::InProgress);
			}
		}

		// try send msg body
		match try!(connection.try_write(msg.get_body())) {
			Some(x) => if x < msg.get_body().len() {
				self.pending_send = Some(msg);
				self.pending_step = 3;
				self.pending_progress = x;
				return Ok(SendStatus::InProgress);
			},
			None => {
				self.pending_send = Some(msg);
				self.pending_step = 3;
				self.pending_progress = 0;
				return Ok(SendStatus::InProgress);
			}
		}

		// TODO : when something is pending, should reregister, to writable at least ...
		Ok(SendStatus::Completed)
	}
}

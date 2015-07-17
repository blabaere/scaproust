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

		// TODO : maybe the connection could be stored inside the Pipe
		// and then referenced from the states inside a &'x Connection field ?

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
	pending_send: Option<SendOperation>
}

impl ConnectedPipeState {

	fn new(id: usize, connection: Rc<RefCell<Box<Connection>>>) -> ConnectedPipeState {
		ConnectedPipeState { 
			id: id,
			connection: connection,
			pending_send: None
		}
	}	

	fn resume_sending(&mut self) -> io::Result<SendStatus> {
		let mut connection = self.connection.borrow_mut();
		let mut operation = self.pending_send.take().unwrap();
		let progress = match try!(operation.advance(&mut **connection)) {
			SendStatus::Postponed(msg) => SendStatus::Postponed(msg),
			SendStatus::Completed => SendStatus::Completed,
			SendStatus::InProgress => {
				self.pending_send = Some(operation);

				SendStatus::InProgress
			}
		};

		Ok(progress)
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
		if self.pending_send.is_some() && events.is_writable() {
			try!(self.resume_sending());
		}

		Ok(None)
	}

	fn send(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>) -> io::Result<SendStatus> {
		self.pending_send = Some(SendOperation::new(msg));

		self.resume_sending()
	}
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum SendOperationStep {
	Prefix,
	Header,
	Body,
	Done
}

struct SendOperation {
	msg: Rc<Message>,
	step: SendOperationStep,
	written: usize
}

impl SendOperation {
	fn new(msg: Rc<Message>) -> SendOperation {
		SendOperation {
			msg: msg,
			step: SendOperationStep::Prefix,
			written: 0
		}
	}

	fn advance(&mut self, connection: &mut Connection) -> io::Result<SendStatus> {
		// try send prefix
		if self.step == SendOperationStep::Prefix {
			let prefix = try!(self.get_prefix_bytes());
			let remaining = prefix.len() - self.written;
			let buffer = &prefix[self.written..];

			match try!(connection.try_write(buffer)) {
				Some(x) => if x < remaining {
					self.written = self.written + x;
					return Ok(SendStatus::InProgress);
				},
				None => {
					return Ok(SendStatus::Postponed(self.msg.clone()));
				}
			}

			self.step = SendOperationStep::Header;
			self.written = 0;
		}

		/*if try!(self.advance_payload_fragment(connection)) {
			self.step = SendOperationStep::Body;
			self.written = 0;
		} else {
			return Ok(SendStatus::InProgress);
		}

		if try!(self.advance_payload_fragment(connection)) {
			self.step = SendOperationStep::Done;
			self.written = 0;
		} else {
			return Ok(SendStatus::InProgress);
		}*/

		// try send msg header
		if self.step == SendOperationStep::Header {
			let header = self.msg.get_header();
			let remaining = header.len() - self.written;

			if remaining > 0 {
				let buffer = &header[self.written..];
				match try!(connection.try_write(buffer)) {
					Some(x) => if x < remaining {
						self.written = self.written + x;
						return Ok(SendStatus::InProgress);
					},
					None => {
						return Ok(SendStatus::InProgress);
					}
				}
			}

			self.step = SendOperationStep::Body;
			self.written = 0;
		}

		// try send msg body
		if self.step == SendOperationStep::Body {
			let body = self.msg.get_body();
			let remaining = body.len() - self.written;

			if remaining > 0 {
				let buffer = &body[self.written..];
				match try!(connection.try_write(buffer)) {
					Some(x) => if x < remaining {
						self.written = self.written + x;
						return Ok(SendStatus::InProgress);
					},
					None => {
						return Ok(SendStatus::InProgress);
					}
				}
			}

			self.step = SendOperationStep::Done;
			self.written = 0;
		};

		Ok(SendStatus::Completed)
	}

	/*fn advance_payload_fragment(&mut self, connection: &mut Connection) -> io::Result<bool> {
		let buffer = match self.step {
			SendOperationStep::Header => self.msg.get_header(),
			_ => self.msg.get_body(),
		};
		let remaining = buffer.len() - self.written;

		let done = if remaining > 0 {
			let fragment = &buffer[self.written..];
			match try!(connection.try_write(fragment)) {
				Some(x) => {
					self.written = self.written + x;

					x == remaining
				},
				None => false
			}
		} else {
			true
		};

		Ok(done)
	}*/

	fn get_prefix_bytes(&self) -> io::Result<Vec<u8>> {
		let mut prefix = Vec::with_capacity(8);
		let msg_len = self.msg.len() as u64;

		try!(prefix.write_u64::<BigEndian>(msg_len));

		Ok(prefix)
	}
	
}

use std::rc::Rc;
use std::cell::RefCell;
use std::sync::mpsc;
use std::io;

use byteorder::{BigEndian, WriteBytesExt};

use mio;

use EventLoop;
use Message;
use event_loop_msg::SocketEvt as SocketEvt;
use protocol::Protocol;
use transport::Connection;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum State {
	Handshake,
	Connected
}

// A pipe is responsible for handshaking with its peer and transfering messages over a connection.
// That means send/receive size prefix and then message payload
// according to the connection readiness and the operation progress
pub struct Pipe {
	addr: Option<String>,
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

	pub fn new(
		token: mio::Token, 
		addr: Option<String>, 
		protocol: &Protocol, 
		connection: Box<Connection>,
		evt_tx: Rc<mpsc::Sender<SocketEvt>>) -> Pipe {

		let conn_ref = Rc::new(RefCell::new(connection));

		Pipe {
			addr: addr,
			state: State::Handshake,
			handshake_state: HandshakePipeState::new(token, protocol, conn_ref.clone()),
			connected_state: ConnectedPipeState::new(token, evt_tx, conn_ref.clone())
		}
	}

	pub fn send(&mut self, msg: Rc<Message>) -> io::Result<SendStatus> {
		self.get_state().send(msg)
	}

	pub fn addr(self) -> Option<String> {
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

	pub fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) -> io::Result<()> {

		if events.is_hup() {
			return Err(io::Error::new(io::ErrorKind::ConnectionReset, "event: hup"));
		}

		if events.is_error() {
			return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "event: error"));
		}

		if let Some(new_state) = try!(self.get_state().ready(event_loop, events)) {
			try!(self.set_state(new_state, event_loop));
		} 

		Ok(())
	}
	
}

// May be it so no a good idea after all
// When handshaking,  the result of ready() can only be bool telling if the handshake is doe or not
// When connected, it has to tell if any message was sent, or received, or both.
trait PipeState {
	fn enter(&mut self, event_loop: &mut EventLoop) -> io::Result<()>;
	fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet)-> io::Result<Option<State>>;

	fn send(&mut self, msg: Rc<Message>) -> io::Result<SendStatus>;
}

struct HandshakePipeState {
	token: mio::Token,
	protocol_id: u16,
	protocol_peer_id: u16,
	connection: Rc<RefCell<Box<Connection>>>,
	sent: bool,
	received: bool
}

impl HandshakePipeState {

	fn new(token: mio::Token, protocol: &Protocol, connection: Rc<RefCell<Box<Connection>>>) -> HandshakePipeState {
		HandshakePipeState { 
			token: token,
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
		debug!("[{:?}] handshake received.", self.token);
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
		debug!("[{:?}] handshake sent.", self.token);
		self.sent = true;
		Ok(())
	}

	fn register(&self, event_loop: &mut EventLoop, interest: mio::EventSet, poll_opt: mio::PollOpt) -> io::Result<()> {
		let connection = self.connection.borrow();
		let fd = connection.as_evented();

		event_loop.register_opt(fd, self.token, interest, poll_opt)
	}

	fn reregister(&self, event_loop: &mut EventLoop, interest: mio::EventSet, poll_opt: mio::PollOpt) -> io::Result<()> {
		let connection = self.connection.borrow();
		let fd = connection.as_evented();

		event_loop.reregister(fd, self.token, interest, poll_opt)
	}

}

impl PipeState for HandshakePipeState {

	fn enter(&mut self, event_loop: &mut EventLoop) -> io::Result<()> {
		debug!("[{:?}] enter handshake state", self.token);
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

	fn send(&mut self, msg: Rc<Message>) -> io::Result<SendStatus> {
		Ok(SendStatus::Postponed(msg))
	}
}

struct ConnectedPipeState {
	token: mio::Token,
	evt_sender: Rc<mpsc::Sender<SocketEvt>>,
	connection: Rc<RefCell<Box<Connection>>>,
	pending_send: Option<SendOperation>
}

impl ConnectedPipeState {

	fn new(token: mio::Token, evt_tx: Rc<mpsc::Sender<SocketEvt>>, connection: Rc<RefCell<Box<Connection>>>) -> ConnectedPipeState {
		ConnectedPipeState { 
			token: token,
			evt_sender: evt_tx,
			connection: connection,
			pending_send: None
		}
	}	

	fn resume_sending(&mut self) -> io::Result<SendStatus> {
		let mut connection = self.connection.borrow_mut();
		let mut operation = self.pending_send.take().unwrap();
		let progress = match try!(operation.send(&mut **connection)) {
			SendStatus::Postponed(msg) => SendStatus::Postponed(msg),
			SendStatus::Completed => {
				let _ = self.evt_sender.send(SocketEvt::MsgSent);
				// TODO : cancel related event loop timeout

				SendStatus::Completed
			},
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
		debug!("[{:?}] enter connected state", self.token);
		let connection = self.connection.borrow_mut();
		let fd = connection.as_evented();
		let interest = mio::EventSet::all();
		let poll_opt = mio::PollOpt::edge();
		try!(event_loop.reregister(fd, self.token, interest, poll_opt));
		Ok(())
	}

	fn ready(&mut self, _: &mut EventLoop, events: mio::EventSet)-> io::Result<Option<State>> {
		if self.pending_send.is_some() && events.is_writable() {
			try!(self.resume_sending());
		}

		Ok(None)
	}

	fn send(&mut self, msg: Rc<Message>) -> io::Result<SendStatus> {
		let operation = try!(SendOperation::new(msg));

		self.pending_send = Some(operation);

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

impl SendOperationStep {
	fn next(&self) -> SendOperationStep {
		match *self {
			SendOperationStep::Prefix => SendOperationStep::Header,
			SendOperationStep::Header => SendOperationStep::Body,
			SendOperationStep::Body => SendOperationStep::Done,
			SendOperationStep::Done => SendOperationStep::Done
		}
	}
}

struct SendOperation {
	prefix: Vec<u8>,
	msg: Rc<Message>,
	step: SendOperationStep,
	written: usize
}

impl SendOperation {
	fn new(msg: Rc<Message>) -> io::Result<SendOperation> {
		let mut prefix = Vec::with_capacity(8);
		let msg_len = msg.len() as u64;

		try!(prefix.write_u64::<BigEndian>(msg_len));

		Ok(SendOperation {
			prefix: prefix,
			msg: msg,
			step: SendOperationStep::Prefix,
			written: 0
		})
	}

	fn step_forward(&mut self) {
		self.step = self.step.next();
		self.written = 0;
	}

	fn send(&mut self, connection: &mut Connection) -> io::Result<SendStatus> {
		// try send size prefix
		if self.step == SendOperationStep::Prefix {
			if try!(self.send_buffer_and_check(connection)) {
				self.step_forward();
			} else {
				if self.written == 0 {
					return Ok(SendStatus::Postponed(self.msg.clone()));
				} else {
					return Ok(SendStatus::InProgress);
				}
			}
		}

		// try send msg header
		if self.step == SendOperationStep::Header {
			if try!(self.send_buffer_and_check(connection)) {
				self.step_forward();
			} else {
				return Ok(SendStatus::InProgress);
			}
		}

		// try send msg body
		if self.step == SendOperationStep::Body {
			if try!(self.send_buffer_and_check(connection)) {
				self.step_forward();
			} else {
				return Ok(SendStatus::InProgress);
			}
		}

		Ok(SendStatus::Completed)
	}

	fn send_buffer_and_check(&mut self, connection: &mut Connection) -> io::Result<bool> {
		let buffer: &[u8] = match self.step {
			SendOperationStep::Prefix => &self.prefix,
			SendOperationStep::Header => self.msg.get_header(),
			SendOperationStep::Body => self.msg.get_body(),
			_ => return Ok(true)
		};

		self.written += try!(self.send_buffer(connection, buffer));

		Ok(self.written == buffer.len())
	}

	fn send_buffer(&self, connection: &mut Connection, buffer: &[u8]) -> io::Result<usize> {
		let remaining = buffer.len() - self.written;

		if remaining > 0 {
			let fragment = &buffer[self.written..];
			let written = match try!(connection.try_write(fragment)) {
				Some(x) => x,
				None => 0
			};

			Ok(written)
		} else {
			Ok(0)
		}
	}

}

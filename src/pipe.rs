use std::rc::Rc;
use std::io;

use byteorder::{BigEndian, WriteBytesExt, ReadBytesExt};

use mio;

use EventLoop;
use Message;
use transport::Connection;
use global;

// A pipe is responsible for handshaking with its peer and transfering messages over a connection.
// That means send/receive size prefix and then message payload
// according to the connection readiness and the operation progress
pub struct Pipe {
	token: mio::Token, 
	addr: Option<String>,
	handshake_state: Option<HandshakePipeState>,
	connected_state: Option<ConnectedPipeState>
}

pub enum SendStatus {
	Postponed(Rc<Message>), // Message can't be sent at the moment : Handshake in progress or would block
    Completed,              // Message has been successfully sent
    InProgress              // Message has been partially sent, will finish later
}

pub enum RecvStatus {
	Postponed,              // Message can't be read at the moment : Handshake in progress or would block
    Completed(Message),     // Message has been successfully read
    InProgress              // Message has been partially read, will finish later
}

impl Pipe {

	pub fn new(
		token: mio::Token, 
		addr: Option<String>, 
		proto_ids: (u16, u16), 
		connection: Box<Connection>) -> Pipe {

		let handshake = HandshakePipeState::new(token, proto_ids, connection);

		Pipe {
			token: token,
			addr: addr,
			handshake_state: Some(handshake),
			connected_state: None
		}
	}

	pub fn open(&self, event_loop: &mut EventLoop) -> io::Result<()> {
		self.handshake_state.as_ref().map_or_else(
			|| Err(global::other_io_error("cannot open pipe after handshake step !")),
			|hs| hs.open(event_loop))
	}

	pub fn close(&self, event_loop: &mut EventLoop) -> io::Result<()> {
		if self.handshake_state.is_some() {
			self.handshake_state.as_ref().unwrap().close(event_loop)
		} else if self.connected_state.is_some() {
			self.connected_state.as_ref().unwrap().close(event_loop)
		} else {
			Ok(())
		}
	}

	pub fn addr(self) -> Option<String> {
		self.addr
	}

	pub fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) -> io::Result<(bool, bool)> {

		if events.is_hup() {
			return Err(io::Error::new(io::ErrorKind::ConnectionReset, "event: hup"));
		}

		if events.is_error() {
			return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "event: error"));
		}

		if self.handshake_state.is_some() {
			let mut handshake = self.handshake_state.take().unwrap();
			if try!(handshake.ready(event_loop, events)) {
				let connection = handshake.connection();
				let connected = ConnectedPipeState::new(self.token, connection);

				try!(connected.open(event_loop));
				self.connected_state = Some(connected)
			} else {
				try!(handshake.reopen(event_loop));
				self.handshake_state = Some(handshake);
			}

			return Ok((false, false));
		} 

		self.connected_state.as_mut().map_or_else(
			|| Err(global::other_io_error("ready notification while pipe is dead")),
			|mut connected| connected.ready(event_loop, events))
	}

	pub fn send(&mut self, msg: Rc<Message>) -> io::Result<SendStatus> {
		if self.handshake_state.is_some() {
			return Ok(SendStatus::Postponed(msg));
		}

		self.connected_state.as_mut().map_or_else(
			|| Err(global::other_io_error("cannot send when pipe is dead")),
			|mut connected| connected.send(msg))		
	}

	pub fn reset_pending_send(&mut self) {
		self.connected_state.as_mut().map(|mut connected| connected.reset_pending_send());
	}

	pub fn on_send_timeout(&mut self) {
		self.connected_state.as_mut().map(|mut connected| connected.on_send_timeout());
	}

	pub fn recv(&mut self) -> io::Result<RecvStatus> {
		if self.handshake_state.is_some() {
			return Ok(RecvStatus::Postponed);
		}

		self.connected_state.as_mut().map_or_else(
			|| Err(global::other_io_error("cannot receive when pipe is dead")),
			|mut connected| connected.recv())		
	}

	pub fn on_recv_timeout(&mut self) {
		self.connected_state.as_mut().map(|mut connected| connected.on_recv_timeout());
	}
}


struct HandshakePipeState {
	token: mio::Token,
	protocol_id: u16,
	protocol_peer_id: u16,
	connection: Box<Connection>,
	sent: bool,
	received: bool
}

impl HandshakePipeState {

	fn new(token: mio::Token, proto_ids: (u16, u16), connection: Box<Connection>) -> HandshakePipeState {
		let (protocol_id, protocol_peer_id) = proto_ids;

		HandshakePipeState { 
			token: token,
			protocol_id: protocol_id,
			protocol_peer_id: protocol_peer_id,
			connection: connection,
			sent: false, 
			received: false }
	}

	fn open(&self, event_loop: &mut EventLoop) -> io::Result<()> {
		event_loop.register_opt(
			self.connection.as_evented(), 
			self.token, 
			mio::EventSet::all(), 
			mio::PollOpt::oneshot())
	}

	fn reopen(&self, event_loop: &mut EventLoop) -> io::Result<()> {
		event_loop.reregister(
			self.connection.as_evented(), 
			self.token, 
			mio::EventSet::all(), 
			mio::PollOpt::oneshot())
	}

	fn close(&self, event_loop: &mut EventLoop) -> io::Result<()> {
		event_loop.deregister(self.connection.as_evented())
	}

	fn ready(&mut self, _: &mut EventLoop, events: mio::EventSet)-> io::Result<bool> {
		if !self.received && events.is_readable() {
			try!(self.read_handshake());
		}

		if !self.sent && events.is_writable() {
			try!(self.write_handshake());
		}

		Ok(self.received && self.sent)
	}

	fn read_handshake(&mut self) -> io::Result<()> {
		let mut handshake = [0u8; 8];
		try!(
			self.connection.try_read(&mut handshake).
			and_then(|_| self.check_received_handshake(&handshake)));
		debug!("[{:?}] handshake received.", self.token);
		self.received = true;
		Ok(())
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

	fn write_handshake(&mut self) -> io::Result<()> {
		// handshake is Zero, 'S', 'P', Version, Proto, Rsvd
		let mut handshake = vec!(0, 83, 80, 0);
		try!(handshake.write_u16::<BigEndian>(self.protocol_id));
		try!(handshake.write_u16::<BigEndian>(0));
		try!(
			self.connection.try_write(&handshake).
			and_then(|w| self.check_sent_handshake(w)));
		debug!("[{:?}] handshake sent.", self.token);
		self.sent = true;
		Ok(())
	}

	fn check_sent_handshake(&self, written: Option<usize>) -> io::Result<()> {
		match written {
			Some(8) => Ok(()),
			Some(_) => Err(io::Error::new(io::ErrorKind::WouldBlock, "failed to send full handshake")),
			_       => Err(io::Error::new(io::ErrorKind::WouldBlock, "failed to send handshake"))
		}
	}

	fn connection(self) -> Box<Connection> {
		self.connection
	}

}

struct ConnectedPipeState {
	token: mio::Token,
	connection: Box<Connection>,
	pending_send: Option<SendOperation>,
	pending_recv: Option<RecvOperation>
}

impl ConnectedPipeState {

	fn new(token: mio::Token, connection: Box<Connection>) -> ConnectedPipeState {
		ConnectedPipeState { 
			token: token,
			connection: connection,
			pending_send: None,
			pending_recv: None
		}
	}

	fn open(&self, event_loop: &mut EventLoop) -> io::Result<()> {
		debug!("[{:?}] enter connected state", self.token);

		event_loop.reregister(
			self.connection.as_evented(),
			self.token,
			mio::EventSet::all(),
			mio::PollOpt::edge())
	}

	fn close(&self, event_loop: &mut EventLoop) -> io::Result<()> {
		event_loop.deregister(self.connection.as_evented())
	}

	fn ready(&mut self, _: &mut EventLoop, events: mio::EventSet)-> io::Result<(bool, bool)> {
		let mut sent = false;
		let mut recv = false;

		if self.pending_send.is_some() && events.is_writable() {
			sent = match try!(self.resume_sending()) {
				SendStatus::Completed => true,
				_                     => false
			}
		}

		if self.pending_recv.is_some() && events.is_readable() {
			recv = match try!(self.resume_receiving()) {
				RecvStatus::Completed(msg) => true,
				_ => false
			}
		}

		Ok((sent, recv))
	}

	fn resume_sending(&mut self) -> io::Result<SendStatus> {
		let mut operation = self.pending_send.take().unwrap();
		let progress = match try!(operation.send(&mut *self.connection)) {
			SendStatus::Postponed(msg) => SendStatus::Postponed(msg),
			SendStatus::Completed      => SendStatus::Completed,
			SendStatus::InProgress     => {
				self.pending_send = Some(operation);

				SendStatus::InProgress
			}
		};

		Ok(progress)
	}

	fn resume_receiving(&mut self) -> io::Result<RecvStatus> {
		debug!("[{:?}] resume_receiving", self.token);
		let mut operation = self.pending_recv.take().unwrap();
		let progress = match try!(operation.recv(&mut *self.connection)) {
			RecvStatus::Postponed      => RecvStatus::Postponed,
			RecvStatus::Completed(msg) => RecvStatus::Completed(msg),
			RecvStatus::InProgress     => {
				self.pending_recv = Some(operation);

				RecvStatus::InProgress
			}
		};

		Ok(progress)
	}

	fn send(&mut self, msg: Rc<Message>) -> io::Result<SendStatus> {
		let operation = try!(SendOperation::new(msg));

		self.pending_send = Some(operation);
		self.resume_sending()
	}

	fn reset_pending_send(&mut self) {
		self.pending_send = None;
	}

	fn on_send_timeout(&mut self) {
		self.pending_send = None;
	}

	fn recv(&mut self) -> io::Result<RecvStatus> {
		let operation = RecvOperation::new();

		self.pending_recv = Some(operation);
		self.resume_receiving()
	}

	fn on_recv_timeout(&mut self) {
		self.pending_recv = None;
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
			SendOperationStep::Body   => SendOperationStep::Done,
			SendOperationStep::Done   => SendOperationStep::Done
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum RecvOperationStep {
	Prefix,
	Payload,
	Done
}

impl RecvOperationStep {
	fn next(&self) -> RecvOperationStep {
		match *self {
			RecvOperationStep::Prefix  => RecvOperationStep::Payload,
			RecvOperationStep::Payload => RecvOperationStep::Done,
			RecvOperationStep::Done    => RecvOperationStep::Done
		}
	}
}

struct RecvOperation {
	prefix: [u8; 8],
    msg_len: u64,
    step: RecvOperationStep,
    buffer: Option<Vec<u8>>,
    read: usize
}

impl RecvOperation {

	fn new() -> RecvOperation {
		RecvOperation {
			prefix: [0u8; 8],
			msg_len: 0,
			step: RecvOperationStep::Prefix,
			buffer: None,
			read: 0
		}
	}

	fn recv(&mut self, connection: &mut Connection) -> io::Result<RecvStatus> {
		if self.step == RecvOperationStep::Prefix {
			debug!("RecvOperation resume to step prefix");
			let mut prefix = [0u8; 8];
			match try!(connection.try_read(&mut prefix)) {
				None    => return Ok(RecvStatus::Postponed),
				Some(0) => return Ok(RecvStatus::Postponed),
				Some(8) => {
					debug!("RecvOperation read 8 bytes prefix");
					let mut bytes: &[u8] = &prefix;
					self.msg_len = try!(bytes.read_u64::<BigEndian>());
					debug!("RecvOperation converted prefix to msg len: {}", self.msg_len);
					self.buffer = Some( vec![0u8; self.msg_len as usize]);
					debug!("RecvOperation allocated buffer");
					self.step = self.step.next();
					debug!("RecvOperation moved step");
				},
				Some(x) => {
					debug!("pipe recv read {} bytes of size prefix ...", x);
					return Ok(RecvStatus::InProgress);
				}
				// TODO fixme : this is wrong, we should keep track of the read byte count
				// and store the size prefix bytes in the struct
			}
		}		

		if self.step == RecvOperationStep::Payload {
			debug!("RecvOperation resume to step payload");
			let mut buffer = self.buffer.take().unwrap();
			let mut read_buffer = [0u8; 1024];
			match try!(connection.try_read(&mut buffer[self.read..])) {
				None    => {
					debug!("pipe recv would block while reading body ...");					
					self.buffer = Some(buffer);
					return Ok(RecvStatus::InProgress);
				},
				Some(0) => {
					debug!("pipe recv read 0 bytes while reading body ...");					
					self.buffer = Some(buffer);
					return Ok(RecvStatus::InProgress);
				},
				Some(x) => {
					self.read = self.read + x;
					if self.read as u64 == self.msg_len {
						self.step = self.step.next();

						return Ok(RecvStatus::Completed(Message::new(buffer)));
					} else {
						debug!("pipe recv read {} more bytes while reading body, total read: {}, expected: {} ...", x, self.read, self.msg_len);					
						self.buffer = Some(buffer);
						return Ok(RecvStatus::InProgress);
					}
				}
			}
		}

		Err(global::other_io_error("recv operation already completed"))
	}

}
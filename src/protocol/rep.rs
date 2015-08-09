use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::collections::HashMap;
use std::io;
use std::boxed::FnBox;

use mio;

use byteorder::{ BigEndian, WriteBytesExt, ReadBytesExt };

use super::Protocol;
use pipe::*;
use global::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use Message;

pub struct Rep {
	pipes: HashMap<mio::Token, RepPipe>,
	evt_sender: Rc<Sender<SocketEvt>>,
	cancel_send_timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>,
	cancel_recv_timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>,
	backtrace: Vec<u8>,
	ttl: u8
}

impl Rep {
	pub fn new(evt_sender: Rc<Sender<SocketEvt>>) -> Rep {
		Rep { 
			pipes: HashMap::new(),
			evt_sender: evt_sender,
			cancel_send_timeout: None,
			cancel_recv_timeout: None,
			backtrace: Vec::with_capacity(64),
			ttl: 8
		}
	}

	fn on_msg_send_ok(&mut self, event_loop: &mut EventLoop) {
		let evt = SocketEvt::MsgSent;
		let timeout = self.cancel_send_timeout.take();

		self.backtrace.clear();
		self.send_event_and_cancel_timeout(event_loop, evt, timeout)
	}

	fn on_msg_send_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
		let evt = SocketEvt::MsgNotSent(err);
		let timeout = self.cancel_send_timeout.take();

		self.backtrace.clear();
		self.send_event_and_cancel_timeout(event_loop, evt, timeout)
	}

	fn on_msg_recv_ok(&mut self, event_loop: &mut EventLoop, msg: Message) {
		self.save_received_header_to_backtrace(&msg);

		let evt = SocketEvt::MsgRecv(msg);
		let timeout = self.cancel_recv_timeout.take();

		self.send_event_and_cancel_timeout(event_loop, evt, timeout)
	}

	fn save_received_header_to_backtrace(&mut self, msg: &Message) {
		self.backtrace.clear();
		self.backtrace.push_all(msg.get_header());
	}

	fn on_msg_recv_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
		let evt = SocketEvt::MsgNotRecv(err);
		let timeout = self.cancel_recv_timeout.take();

		self.send_event_and_cancel_timeout(event_loop, evt, timeout)
	}

	fn send_event_and_cancel_timeout(&self, 
		event_loop: &mut EventLoop, 
		evt: SocketEvt, 
		timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>) {
		
		let _ = self.evt_sender.send(evt);

		timeout.map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));
	}

	fn raw_msg_to_msg(&mut self, pipe_token: mio::Token, raw_msg: Message) -> io::Result<Option<Message>> {
		let (mut header, mut body) = raw_msg.explode();
		let pipe_id = pipe_token.as_usize() as u32;

		header.reserve(4);
		try!(header.write_u32::<BigEndian>(pipe_id));

		let mut hops = 0;
		loop {
			if hops >= self.ttl {
				return Ok(None);
			}

			hops += 1;

			if body.len() < 4 {
				return Ok(None);
			}

			let tail = body.split_off(4);
			header.push_all(&body);

			let position = header.len() - 4;
			if header[position] & 0x80 != 0 {
				let msg = Message::with_header_and_body(header, tail);

				return Ok(Some(msg));
			}
			body = tail;
		}
	}

	fn msg_to_raw_msg(&mut self, msg: Message) -> io::Result<(Message, mio::Token)> {
		let (header, body) = msg.explode();
		let pipe_token = try!(self.restore_pipe_id_from_backtrace());
		let header = try!(self.restore_header_from_backtrace(header));

		Ok((Message::with_header_and_body(header, body), pipe_token))
	}

	fn restore_pipe_id_from_backtrace(&self) -> io::Result<mio::Token> {
		if self.backtrace.len() < 4 {
			return Err(other_io_error("no backtrace from received message"));
		}
		let pipe_id_bytes = vec!(
			self.backtrace[0],
			self.backtrace[1],
			self.backtrace[2],
			self.backtrace[3]
		);
		let mut pipe_id_reader = io::Cursor::new(pipe_id_bytes);
		let pipe_id = try!(pipe_id_reader.read_u32::<BigEndian>());
		let pipe_token = mio::Token(pipe_id as usize);

		Ok(pipe_token)
	}

	fn restore_header_from_backtrace(&self, mut header: Vec<u8>) -> io::Result<Vec<u8>> {
		if self.backtrace.len() < 8 {
			return Err(other_io_error("no header in backtrace from received message"));
		}

		let backtrace = &self.backtrace[4..];
		
		header.reserve(backtrace.len());
		header.push_all(backtrace);

		Ok(header)
	}

	fn on_raw_msg_recv(&mut self, event_loop: &mut EventLoop, pipe_token: mio::Token, raw_msg: Message) {
		match self.raw_msg_to_msg(pipe_token, raw_msg) {
			Ok(None)      => {}
			Ok(Some(msg)) => self.on_msg_recv_ok(event_loop, msg),
			Err(e)        => self.on_msg_recv_err(event_loop, e)
		}
	}
}

impl Protocol for Rep {
	fn id(&self) -> u16 {
		SocketType::Rep.id()
	}

	fn peer_id(&self) -> u16 {
		SocketType::Req.id()
	}

	fn add_pipe(&mut self, token: mio::Token, pipe: Pipe) {
		self.pipes.insert(token, RepPipe::new(pipe));
	}

	fn remove_pipe(&mut self, token: mio::Token) -> Option<Pipe> {
		self.pipes.remove(&token).map(|p| p.remove())
	}

	fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
		let mut msg_sending = false;
		let mut msg_sent = false;
		let mut received_msg = None;
		let mut receiving_msg = false;

		if let Some(pipe) = self.pipes.get_mut(&token) {
			let (sent, received) = try!(pipe.ready(event_loop, events));

			if sent {
				msg_sent = true;
			} else {
				match try!(pipe.resume_pending_send()) {
					Some(true)  => msg_sent = true,
					Some(false) => msg_sending = true,
					None        => {}
				}
			}

			if received.is_some() {
				received_msg = received;
			} else {
				match try!(pipe.resume_pending_recv()) {
					Some(RecvStatus::Completed(msg))   => received_msg = Some(msg),
					Some(RecvStatus::InProgress)       => receiving_msg = true,
					Some(RecvStatus::Postponed) | None => {}
				}
			}
		}

		if msg_sent {
			self.on_msg_send_ok(event_loop);
		}

		if msg_sending | msg_sent {
			for (_, pipe) in self.pipes.iter_mut() {
				pipe.reset_pending_send();
			}
		}

		if received_msg.is_some() | receiving_msg {
			for (_, pipe) in self.pipes.iter_mut() {
				pipe.reset_pending_recv();
			}
		}

		if received_msg.is_some() {
			let raw_msg = received_msg.unwrap();

			self.on_raw_msg_recv(event_loop, token, raw_msg);
		}

		Ok(())
	}

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>) {
		self.cancel_send_timeout = Some(cancel_timeout);

		let (raw_msg, pipe_token) = match self.msg_to_raw_msg(msg) {
			Err(e) => {
				self.on_msg_send_err(event_loop, e);
				return;
			},
			Ok((raw_msg, pipe_token)) => (raw_msg, pipe_token)
		};

		let mut sent = false;
		let mut sending = false;
		let mut pending = false;
		let shared_msg = Rc::new(raw_msg);

		if let Some(pipe) = self.pipes.get_mut(&pipe_token) {
			match pipe.send(shared_msg.clone()) {
				Ok(Some(true))  => sent = true,
				Ok(Some(false)) => sending = true,
				Ok(None)        => pending = true,
				Err(_)          => {}
				// this pipe looks dead, but it will be taken care of during next ready notification
			}
		}

		if sent {
			self.on_msg_send_ok(event_loop);
		}

		if sent | sending {
			for (_, pipe) in self.pipes.iter_mut() {
				pipe.reset_pending_send();
			}
		} else if pending == false {
			let err = io::Error::new(io::ErrorKind::NotConnected, "no connected endpoint");

			self.on_msg_send_err(event_loop, err);
		}
	}

	fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
		let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

		self.on_msg_send_err(event_loop, err);

		for (_, pipe) in self.pipes.iter_mut() {
			pipe.on_send_timeout();
		}
	}

	fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>) {
		self.cancel_recv_timeout = Some(cancel_timeout);

		let mut received = None;
		let mut received_from = None;
		let mut receiving = false;
		let mut pending = false;

		for (token, pipe) in self.pipes.iter_mut() {
			match pipe.recv() {
				Ok(RecvStatus::Completed(msg)) => {
					received = Some(msg);
					received_from = Some(token.clone());
				},
				Ok(RecvStatus::InProgress)     => receiving = true,
				Ok(RecvStatus::Postponed)      => pending = true,
				Err(_)                         => continue
			}

			if received.is_some() | receiving {
				break;
			}
		}

		if received.is_some() | receiving {
			for (_, pipe) in self.pipes.iter_mut() {
				pipe.reset_pending_recv();
			}
		} else if pending == false {
			let err = io::Error::new(io::ErrorKind::NotConnected, "no connected endpoint");

			self.on_msg_recv_err(event_loop, err);
		}

		if received.is_some() && received_from.is_some() {
			let pipe_token = received_from.unwrap();
			let raw_msg = received.unwrap();

			self.on_raw_msg_recv(event_loop, pipe_token, raw_msg);
		}
	}

	fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
		let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

		self.on_msg_recv_err(event_loop, err);

		for (_, pipe) in self.pipes.iter_mut() {
			pipe.on_recv_timeout();
		}
	}
}

struct RepPipe {
    pipe: Pipe,
    pending_send: Option<Rc<Message>>,
    pending_recv: bool
}

impl RepPipe {
	fn new(pipe: Pipe) -> RepPipe {
		RepPipe { 
			pipe: pipe,
			pending_send: None,
			pending_recv: false
		}
	}

	fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) -> io::Result<(bool, Option<Message>)> {
		self.pipe.ready(event_loop, events)
	}

	fn send(&mut self, msg: Rc<Message>) -> io::Result<Option<bool>> {
		let progress = match try!(self.pipe.send(msg)) {
			SendStatus::Completed => {
				self.pipe.cancel_sending();
				self.pending_send = None;
				Some(true)
			},
			SendStatus::InProgress => {
				self.pending_send = None;
				Some(false)
			},
			SendStatus::Postponed(message) => {
				self.pipe.cancel_sending();
				self.pending_send = Some(message);
				None
			}
		};

		Ok(progress)
	}

	fn on_send_timeout(&mut self) {
		self.pending_send = None;
		self.pipe.cancel_sending();
	}

	fn resume_pending_send(&mut self) -> io::Result<Option<bool>> {
		match self.pending_send.take() {
			None      => Ok(None),
			Some(msg) => self.send(msg)
		}
	}

	fn reset_pending_send(&mut self) {
		self.pending_send = None;
	}

	fn recv(&mut self) -> io::Result<RecvStatus> {
		let progress = try!(self.pipe.recv());

		match progress {
			RecvStatus::Completed(_) => {
				self.pipe.cancel_receiving();
				self.pending_recv = false;
			},
			RecvStatus::InProgress => {
				self.pending_recv = false;
			},
			RecvStatus::Postponed => {
				self.pipe.cancel_receiving();
				self.pending_recv = true;
			}
		}

		Ok(progress)
	}

	fn on_recv_timeout(&mut self) {
		self.pending_recv = false;
		self.pipe.cancel_receiving();
	}

	fn resume_pending_recv(&mut self) -> io::Result<Option<RecvStatus>> {
		let result = if self.pending_recv {
			let status = try!(self.pipe.recv());
			Some(status)
		} else {
			None
		};

		Ok(result)
	}

	fn reset_pending_recv(&mut self) {
		self.pending_recv = false;
	}

	fn remove(self) -> Pipe {
		self.pipe
	}
}
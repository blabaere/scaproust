use std::rc::Rc;
use std::sync::mpsc;
use std::collections::HashMap;
use std::io;
use std::boxed::FnBox;

use time;

use mio;

use byteorder::{ BigEndian, WriteBytesExt, ReadBytesExt };

use super::Protocol;
use pipe::*;
use global::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use Message;

pub struct Req {
	pipes: HashMap<mio::Token, ReqPipe>,
	evt_sender: Rc<mpsc::Sender<SocketEvt>>,
	cancel_send_timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>,
	cancel_recv_timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>,
	pending_req_id: Option<u32>,
	req_id_seq: u32
}

impl Req {
	pub fn new(evt_sender: Rc<mpsc::Sender<SocketEvt>>) -> Req {
		Req { 
			pipes: HashMap::new(),
			evt_sender: evt_sender,
			cancel_send_timeout: None,
			cancel_recv_timeout: None,
			pending_req_id: None,
			req_id_seq: time::get_time().nsec as u32
		}
	}

	fn on_msg_send_ok(&mut self, event_loop: &mut EventLoop) {
		let evt = SocketEvt::MsgSent;
		let timeout = self.cancel_send_timeout.take();

		self.send_event_and_cancel_timeout(event_loop, evt, timeout)
	}

	fn on_msg_send_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
		let evt = SocketEvt::MsgNotSent(err);
		let timeout = self.cancel_send_timeout.take();

		self.send_event_and_cancel_timeout(event_loop, evt, timeout)
	}

	fn on_msg_recv_ok(&mut self, event_loop: &mut EventLoop, msg: Message) {
		let evt = SocketEvt::MsgRecv(msg);
		let timeout = self.cancel_recv_timeout.take();

		self.pending_req_id = None;
		self.send_event_and_cancel_timeout(event_loop, evt, timeout)
	}

	fn on_msg_recv_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
		let evt = SocketEvt::MsgNotRecv(err);
		let timeout = self.cancel_recv_timeout.take();

		self.pending_req_id = None;
		self.send_event_and_cancel_timeout(event_loop, evt, timeout)
	}

	fn send_event_and_cancel_timeout(&self, 
		event_loop: &mut EventLoop, 
		evt: SocketEvt, 
		timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>) {
		
		let _ = self.evt_sender.send(evt);

		timeout.map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));
	}

	fn next_req_id(&mut self) -> u32 {
		let next_id = self.req_id_seq | 0x80000000;

		self.req_id_seq += 1;

		next_id
	}

	fn raw_msg_to_msg(&mut self, raw_msg: Message) -> io::Result<(Message, u32)> {
		let mut body = raw_msg.to_buffer();
		let mut header = Vec::with_capacity(4);

		for _ in 0..4 {
			header.push(body.remove(0));
		}

		let mut bytes: &[u8] = &header;
		let req_id = try!(bytes.read_u32::<BigEndian>());
		let msg = Message::with_body(body);

		Ok((msg, req_id))
	}

	fn msg_to_raw_msg(&mut self, msg: Message, req_id: u32) -> io::Result<Message> {
		let mut raw_msg = msg;

		raw_msg.header.reserve(4);
		try!(raw_msg.header.write_u32::<BigEndian>(req_id));

		Ok(raw_msg)
	}

	fn on_raw_msg_recv(&mut self, event_loop: &mut EventLoop, raw_msg: Message) {
		if raw_msg.get_body().len() < 4 {
			return;
		}

		match self.raw_msg_to_msg(raw_msg) {
			Ok((msg, req_id)) => {
				let expected_id = self.pending_req_id.take().unwrap();

				if req_id == expected_id {
					self.on_msg_recv_ok(event_loop, msg);
				} else {
					self.pending_req_id = Some(expected_id);
				}
			},
			Err(e) => {
				self.on_msg_recv_err(event_loop, e);
			}
		}
	}
}

impl Protocol for Req {
	fn id(&self) -> u16 {
		SocketType::Req.id()
	}

	fn peer_id(&self) -> u16 {
		SocketType::Rep.id()
	}

	fn add_pipe(&mut self, token: mio::Token, pipe: Pipe) {
		self.pipes.insert(token, ReqPipe::new(pipe));
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
				pipe.reset_pending_send();
			}
		}

		if received_msg.is_some() && self.pending_req_id.is_some() {
			let raw_msg = received_msg.unwrap();

			self.on_raw_msg_recv(event_loop, raw_msg);
		}

		Ok(())
	}

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>) {
		let req_id = self.next_req_id();

		self.cancel_send_timeout = Some(cancel_timeout);
		self.pending_req_id = Some(req_id);

		let raw_msg = match self.msg_to_raw_msg(msg, req_id) {
			Err(e) => {
				self.on_msg_send_err(event_loop, e);
				return;
			},
			Ok(raw_msg) => raw_msg
		};

		let mut sent = false;
		let mut sending = false;
		let mut pending = false;
		let shared_msg = Rc::new(raw_msg);

		for (_, pipe) in self.pipes.iter_mut() {
			match pipe.send(shared_msg.clone()) {
				Ok(Some(true))  => sent = true,
				Ok(Some(false)) => sending = true,
				Ok(None)        => pending = true,
				Err(_)          => continue 
				// this pipe looks dead, but it will be taken care of during next ready notification
			}

			if sent | sending {
				break;
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
		let mut receiving = false;
		let mut pending = false;

		for (_, pipe) in self.pipes.iter_mut() {
			match pipe.recv() {
				Ok(RecvStatus::Completed(msg)) => received = Some(msg),
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

		if received.is_some() && self.pending_req_id.is_some() {
			let raw_msg = received.unwrap();

			self.on_raw_msg_recv(event_loop, raw_msg);
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

struct ReqPipe {
    pipe: Pipe,
    pending_send: Option<Rc<Message>>,
    pending_recv: bool
}

impl ReqPipe {
	fn new(pipe: Pipe) -> ReqPipe {
		ReqPipe { 
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
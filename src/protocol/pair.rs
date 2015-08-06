use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::io;
use std::boxed::FnBox;

use mio;

use super::Protocol;
use pipe::*;
use global::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use Message;

pub struct Pair {
	pipe: Option<PairPipe>,
	evt_sender: Rc<Sender<SocketEvt>>,
	cancel_send_timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>,
	cancel_recv_timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>
}

impl Pair {
	pub fn new(evt_sender: Rc<Sender<SocketEvt>>) -> Pair {
		Pair { 
			pipe: None,
			evt_sender: evt_sender,
			cancel_send_timeout: None,
			cancel_recv_timeout: None
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

		self.send_event_and_cancel_timeout(event_loop, evt, timeout)
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
}

impl Protocol for Pair {
	fn id(&self) -> u16 {
		SocketType::Pair.id()
	}

	fn peer_id(&self) -> u16 {
		SocketType::Pair.id()
	}

	fn add_pipe(&mut self, token: mio::Token, pipe: Pipe) {
		self.pipe = Some(PairPipe::new(token, pipe));
	}

	fn remove_pipe(&mut self, token: mio::Token) -> Option<Pipe> {
		if Some(token) == self.pipe.as_ref().map(|p| p.token()) {
			self.pipe.take().map(|p| p.remove())
		} else {
			None
		}
	}

	fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
		if self.pipe.is_none() {
			return Ok(());
		}
		if Some(token) != self.pipe.as_ref().map(|p| p.token()) {
			return Ok(());
		}

		let mut pipe = self.pipe.take().unwrap();
		let mut msg_sending = false;
		let mut msg_sent = false;
		let mut received_msg = None;
		let mut receiving_msg = false;

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

		if msg_sent {
			self.on_msg_send_ok(event_loop);
		}

		if msg_sending | msg_sent {
			pipe.reset_pending_send();
		}

		if received_msg.is_some() | receiving_msg {
			pipe.reset_pending_recv();
		}

		if received_msg.is_some() {
			self.on_msg_recv_ok(event_loop, received_msg.unwrap());
		}

		self.pipe = Some(pipe);

		Ok(())
	}

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>) {
		if self.pipe.is_none() {
			let err = io::Error::new(io::ErrorKind::NotConnected, "no connected endpoint");

			self.on_msg_send_err(event_loop, err);

			return;
		}

		let mut pipe = self.pipe.take().unwrap();

		self.cancel_send_timeout = Some(cancel_timeout);

		let mut sent = false;
		let mut sending = false;
		let mut pending = false;

		match pipe.send(Rc::new(msg)) {
			Ok(Some(true))  => sent = true,
			Ok(Some(false)) => sending = true,
			Ok(None)        => pending = true,
			Err(_)          => {} 
			// this pipe looks dead, but it will be taken care of during next ready notification
		}

		if sent {
			self.on_msg_send_ok(event_loop);
		}

		if sent | sending {
			pipe.reset_pending_send();
		} else if pending == false {
			let err = io::Error::new(io::ErrorKind::NotConnected, "no connected endpoint");

			self.on_msg_send_err(event_loop, err);
		}

		self.pipe = Some(pipe);
	}

	fn on_send_timeout(&mut self, event_loop: &mut EventLoop) {
		let err = io::Error::new(io::ErrorKind::TimedOut, "send timeout reached");

		self.on_msg_send_err(event_loop, err);

		if let Some(pipe) = self.pipe.as_mut() {
			pipe.on_send_timeout();
		}
	}

	fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>) {
		if self.pipe.is_none() {
			let err = io::Error::new(io::ErrorKind::NotConnected, "no connected endpoint");

			self.on_msg_send_err(event_loop, err);

			return;
		}

		let mut pipe = self.pipe.take().unwrap();

		self.cancel_recv_timeout = Some(cancel_timeout);

		let mut received = None;
		let mut receiving = false;
		let mut pending = false;

		match pipe.recv() {
			Ok(RecvStatus::Completed(msg)) => received = Some(msg),
			Ok(RecvStatus::InProgress)     => receiving = true,
			Ok(RecvStatus::Postponed)      => pending = true,
			Err(_)                         => {}
		}

		if received.is_some() | receiving {
			pipe.reset_pending_recv();
		} else if pending == false {
			let err = io::Error::new(io::ErrorKind::NotConnected, "no connected endpoint");

			self.on_msg_recv_err(event_loop, err);
		}

		if received.is_some() {
			self.on_msg_recv_ok(event_loop, received.unwrap());
		}

		self.pipe = Some(pipe);
	}
	
	fn on_recv_timeout(&mut self, event_loop: &mut EventLoop) {
		let err = io::Error::new(io::ErrorKind::TimedOut, "recv timeout reached");

		self.on_msg_recv_err(event_loop, err);

		if let Some(pipe) = self.pipe.as_mut() {
			pipe.on_recv_timeout();
		}
	}
}

struct PairPipe {
	token: mio::Token,
    pipe: Pipe,
    pending_send: Option<Rc<Message>>,
    pending_recv: bool
}

impl PairPipe {
	fn new(token: mio::Token, pipe: Pipe) -> PairPipe {
		PairPipe { 
			token: token,
			pipe: pipe,
			pending_send: None,
			pending_recv: false
		}
	}

	fn token(&self) -> mio::Token {
		self.token
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
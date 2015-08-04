use std::rc::Rc;
use std::sync::mpsc;
use std::collections::HashMap;
use std::io;
use std::boxed::FnBox;

use mio;

use super::Protocol;
use pipe::*;
use global::SocketType as SocketType;
use event_loop_msg::SocketEvt as SocketEvt;
use EventLoop;
use Message;

pub struct Push {
	pipes: HashMap<mio::Token, PushPipe>,
	evt_sender: Rc<mpsc::Sender<SocketEvt>>,
	cancel_timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>
}

impl Push {
	pub fn new(evt_sender: Rc<mpsc::Sender<SocketEvt>>) -> Push {
		Push { 
			pipes: HashMap::new(),
			evt_sender: evt_sender,
			cancel_timeout: None
		}
	}

	fn on_msg_send_ok(&mut self, event_loop: &mut EventLoop) {
		let _ = self.evt_sender.send(SocketEvt::MsgSent);

		self.cancel_timeout.take().map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));
	}

	fn on_msg_send_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
		let _ = self.evt_sender.send(SocketEvt::MsgNotSent(err));

		self.cancel_timeout.take().map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));
	}
}

impl Protocol for Push {
	fn id(&self) -> u16 {
		SocketType::Push.id()
	}

	fn peer_id(&self) -> u16 {
		SocketType::Pull.id()
	}

	fn add_pipe(&mut self, token: mio::Token, pipe: Pipe) {
		self.pipes.insert(token, PushPipe::new(pipe));
	}

	fn remove_pipe(&mut self, token: mio::Token) -> Option<Pipe> {
		self.pipes.remove(&token).map(|p| p.remove())
	}

	fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
		let mut msg_acquired = false;
		let mut msg_sent = false;

		if let Some(pipe) = self.pipes.get_mut(&token) {
			let (sent, _) = try!(pipe.ready(event_loop, events));

			if sent {
				msg_sent = true;
			} else {
				match try!(pipe.resume_pending_send()) {
					Some(true)  => msg_sent = true,
					Some(false) => msg_acquired = true,
					None        => msg_acquired = false
				}
			}
		}

		if msg_sent {
			self.on_msg_send_ok(event_loop);
		}

		if msg_acquired | msg_sent {
			for (_, pipe) in self.pipes.iter_mut() {
				pipe.reset_pending_send();
			}
		}

		Ok(())
	}

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>) {
		self.cancel_timeout = Some(cancel_timeout);

		let mut sent = false;
		let mut piped = false;
		let mut shared = false;
		let shared_msg = Rc::new(msg);

		for (_, pipe) in self.pipes.iter_mut() {
			match pipe.send(shared_msg.clone()) {
				Ok(Some(true))  => sent = true,
				Ok(Some(false)) => piped = true,
				Ok(None)        => shared = true,
				Err(_)          => continue 
				// this pipe looks dead, but it will be taken care of during next ready notification
			}

			if sent | piped {
				break;
			}
		}

		if sent {
			self.on_msg_send_ok(event_loop);
		}

		if sent | piped {
			for (_, pipe) in self.pipes.iter_mut() {
				pipe.reset_pending_send();
			}
		} else if shared == false {
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
		//let err = global::other_io_error("recv not supported by protocol");
		//let cmd = SocketEvt::MsgNotRecv(err);
		//let _ = self.evt_sender.send(cmd);
	}
	
	fn on_recv_timeout(&mut self, _: &mut EventLoop) {
	}
}

struct PushPipe {
    pipe: Pipe,
    pending_send: Option<Rc<Message>>
}

impl PushPipe {
	fn new(pipe: Pipe) -> PushPipe {
		PushPipe { 
			pipe: pipe,
			pending_send: None
		}
	}

	fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) -> io::Result<(bool, bool)> {
		self.pipe.ready(event_loop, events)
	}

	fn send(&mut self, msg: Rc<Message>) -> io::Result<Option<bool>> {
		let progress = match try!(self.pipe.send(msg)) {
			SendStatus::Completed => Some(true),
			SendStatus::InProgress => Some(false),
			SendStatus::Postponed(message) => {
				self.pipe.reset_pending_send();
				self.pending_send = Some(message);
				None
			}
		};

		Ok(progress)
	}

	fn on_send_timeout(&mut self) {
		self.pending_send = None;
		self.pipe.on_send_timeout();
	}

	fn resume_pending_send(&mut self) -> io::Result<Option<bool>> {
		match self.pending_send.take() {
			None => Ok(None),
			Some(msg) => self.send(msg)
		}
	}

	fn reset_pending_send(&mut self) {
		self.pipe.reset_pending_send();
		self.pending_send = None;
	}

	fn remove(self) -> Pipe {
		self.pipe
	}
}
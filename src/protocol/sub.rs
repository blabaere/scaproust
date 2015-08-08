use std::rc::Rc;
use std::sync::mpsc;
use std::collections::HashMap;
use std::io;
use std::boxed::FnBox;

use mio;

use super::Protocol;
use pipe::*;
use global::*;
use event_loop_msg::SocketEvt;
use EventLoop;
use Message;

pub struct Sub {
	pipes: HashMap<mio::Token, SubPipe>,
	evt_sender: Rc<mpsc::Sender<SocketEvt>>,
	cancel_timeout: Option<Box<FnBox(&mut EventLoop)-> bool>>
}

impl Sub {
	pub fn new(evt_sender: Rc<mpsc::Sender<SocketEvt>>) -> Sub {
		Sub { 
			pipes: HashMap::new(),
			evt_sender: evt_sender,
			cancel_timeout: None
		}
	}

	fn on_msg_recv_ok(&mut self, event_loop: &mut EventLoop, msg: Message) {
		let _ = self.evt_sender.send(SocketEvt::MsgRecv(msg));

		self.cancel_timeout.take().map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));
	}

	fn on_msg_recv_err(&mut self, event_loop: &mut EventLoop, err: io::Error) {
		let _ = self.evt_sender.send(SocketEvt::MsgNotRecv(err));

		self.cancel_timeout.take().map(|cancel_timeout| cancel_timeout.call_box((event_loop,)));
	}
}

impl Protocol for Sub {
	fn id(&self) -> u16 {
		SocketType::Sub.id()
	}

	fn peer_id(&self) -> u16 {
		SocketType::Pub.id()
	}

	fn add_pipe(&mut self, token: mio::Token, pipe: Pipe) {
		self.pipes.insert(token, SubPipe::new(pipe));
	}

	fn remove_pipe(&mut self, token: mio::Token) -> Option<Pipe> {
		self.pipes.remove(&token).map(|p| p.remove())
	}

	fn ready(&mut self, event_loop: &mut EventLoop, token: mio::Token, events: mio::EventSet) -> io::Result<()> {
		let mut received_msg = None;
		let mut receiving_msg = false;

		if let Some(pipe) = self.pipes.get_mut(&token) {
			let received = try!(pipe.ready(event_loop, events));

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

		// TODO filter message against subscriptions here
		// try again if message does not match

		if received_msg.is_some() | receiving_msg {
			for (_, pipe) in self.pipes.iter_mut() {
				pipe.reset_pending_recv();
			}
		}

		if received_msg.is_some() {
			self.on_msg_recv_ok(event_loop, received_msg.unwrap());
		}

		Ok(())
	}

	fn send(&mut self, _: &mut EventLoop, _: Message, _: Box<FnBox(&mut EventLoop)-> bool>) {
		let err = other_io_error("send not supported by protocol");
		let cmd = SocketEvt::MsgNotSent(err);
		let _ = self.evt_sender.send(cmd);
	}

	fn on_send_timeout(&mut self, _: &mut EventLoop) {
	}

	fn recv(&mut self, event_loop: &mut EventLoop, cancel_timeout: Box<FnBox(&mut EventLoop)-> bool>) {
		self.cancel_timeout = Some(cancel_timeout);

		let mut received = None;
		let mut receiving = false;
		let mut pending = false;

		for (_, pipe) in self.pipes.iter_mut() {
			match pipe.recv() {
				Ok(RecvStatus::Completed(msg)) => {
					// TODO filter message against subscriptions here ?
					// try again if message does not match
					received = Some(msg)
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

		if received.is_some() {
			self.on_msg_recv_ok(event_loop, received.unwrap());
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

struct SubPipe {
    pipe: Pipe,
    pending_recv: bool
}

impl SubPipe {
	fn new(pipe: Pipe) -> SubPipe {
		SubPipe { 
			pipe: pipe,
			pending_recv: false
		}
	}

	fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) -> io::Result<Option<Message>> {
		let (_, received) = try!(self.pipe.ready(event_loop, events));

		Ok(received)
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
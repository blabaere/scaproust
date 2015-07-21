use std::rc::Rc;
use std::sync::mpsc;
use std::collections::HashMap;
use std::io;
use mio;

use super::Protocol;
use pipe::*;
use global::SocketType as SocketType;
use event_loop_msg::SocketEvt as SocketEvt;
use EventLoop;
use Message;

pub struct Push {
	pipes: HashMap<usize, PushPipe>,
	evt_sender: Rc<mpsc::Sender<SocketEvt>>
}

impl Push {
	pub fn new(evt_sender: Rc<mpsc::Sender<SocketEvt>>) -> Push {
		Push { 
			pipes: HashMap::new(),
			evt_sender: evt_sender
		}
	}
}

impl Protocol for Push {
	fn id(&self) -> u16 {
		SocketType::Push.id()
	}

	fn peer_id(&self) -> u16 {
		SocketType::Pull.id()
	}

	fn add_pipe(&mut self, id: usize, pipe: Pipe) {
		self.pipes.insert(id, PushPipe::new(pipe));
	}

	fn remove_pipe(&mut self, id: usize) -> Option<String> {
		self.pipes.remove(&id).map(|p| p.addr())
	}

	fn ready(&mut self, event_loop: &mut EventLoop, id: usize, events: mio::EventSet) -> io::Result<()> {
		let mut clear_pending_send = false;

		if let Some(pipe) = self.pipes.get_mut(&id) {
			try!(pipe.ready(event_loop, events));

			clear_pending_send = try!(pipe.flush_pending_send()).is_some();
		}

		if clear_pending_send {
			for (_, pipe) in self.pipes.iter_mut() {
				pipe.clear_pending_send();
			}
		}

		Ok(())
	}

	fn send(&mut self, _: &mut EventLoop, msg: Message) {
		let mut sent = false;
		let mut piped = false;
		let mut shared = false;
		let shared_msg = Rc::new(msg);

		for (_, pipe) in self.pipes.iter_mut() {
			match pipe.send(shared_msg.clone()) {
				Ok(Some(true)) => sent = true,
				Ok(Some(false)) => piped = true,
				Ok(None) => shared = true,
				Err(_) => continue 
				// this pipe looks dead, but it will be taken care of during next ready notification
			}

			if sent | piped {
				break;
			}
		}

		if sent | piped {
			for (_, pipe) in self.pipes.iter_mut() {
				pipe.clear_pending_send();
			}
		} else if shared == false {
			let err = io::Error::new(io::ErrorKind::NotConnected, "no connected endpoint");
			let _ = self.evt_sender.send(SocketEvt::MsgNotSent(err));
			// TODO : cancel related event loop timeout
		}
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

	fn ready(&mut self, event_loop: &mut EventLoop, events: mio::EventSet) -> io::Result<()> {
		self.pipe.ready(event_loop, events)
	}

	fn send(&mut self, msg: Rc<Message>) -> io::Result<Option<bool>> {
		let progress = match try!(self.pipe.send(msg)) {
			SendStatus::Completed => Some(true),
			SendStatus::InProgress => Some(false),
			SendStatus::Postponed(message) => {
				self.pending_send = Some(message);
				None
			}
		};

		Ok(progress)
	}

	fn flush_pending_send(&mut self) -> io::Result<Option<bool>> {
		match self.pending_send.take() {
			None => Ok(None),
			Some(msg) => self.send(msg)
		}
	}

	fn clear_pending_send(&mut self) {
		self.pending_send = None;
	}

	fn addr(self) -> String {
		self.pipe.addr()
	}
}
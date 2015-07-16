use std::rc::*;
use std::collections::HashMap;
use std::io;
use mio;

use super::Protocol;
use pipe::*;
use global::SocketType as SocketType;
use EventLoop;
use Message;

pub struct Push {
	pipes: HashMap<usize, PushPipe> 
}

impl Push {
	pub fn new() -> Push {
		Push { 
			pipes: HashMap::new()
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
		if let Some(pipe) = self.pipes.get_mut(&id) {
			pipe.ready(event_loop, events)
			// TODO check if a pending message was sent
		} else {
			Ok(())
		}
	}

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message) {
		let mut sent = false;
		let mut piped = false;
		let mut shared = false;
		let shared_msg = Rc::new(msg);

		// after loop state can be
		// - message sent : clean push pipes and notify success upstream
		// - message partially sent : clean push pipes and wait some ...
		// - message pending : wait some ...
		// - message not acquired by anybody (everybody in error !!!) : notify failure upstream 

		for (_, pipe) in self.pipes.iter_mut() {
			match pipe.send(event_loop, shared_msg.clone()) {
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

		if sent {
			info!("Message sent.");
		} else if piped {
			info!("Message sending in progress.");
		} else if shared {
			info!("Message sending postponed.");
		} else {
			info!("Message NOT sent")
		}

		if sent | piped {
			for (_, pipe) in self.pipes.iter_mut() {
				pipe.clean_pending_send();
			}
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
		// TODO check if there is a pending message to be sent ...
	}

	// result can be
	// - sent    (msg can be dropped)
	// - sending (msg acquired by pipe)
	// - pending (msg shared with other push pipes)
	fn send(&mut self, event_loop: &mut EventLoop, msg: Rc<Message>) -> io::Result<Option<bool>> {
		let progress = match try!(self.pipe.send(event_loop, msg)) {
			SendStatus::Completed => Some(true),
			SendStatus::InProgress => Some(false),
			SendStatus::Postponed(message) => {
				self.pending_send = Some(message);
				None
			}
		};

		Ok(progress)
	}

	fn clean_pending_send(&mut self) {
		self.pending_send = None;
	}

	fn addr(self) -> String {
		self.pipe.addr()
	}
}
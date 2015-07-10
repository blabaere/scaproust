use std::collections::HashMap;

use mio;

use super::Protocol;
use pipe::Pipe as Pipe;
use pipe::PipeStateIdx as PipeStateIdx;
use global::SocketType as SocketType;
use EventLoop;
use Message;

pub struct Push {
	pipes: HashMap<usize, Pipe> 
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
		self.pipes.insert(id, pipe);
	}

	fn ready(&mut self, event_loop: &mut EventLoop, id: usize, events: mio::EventSet) {
		if let Some(pipe) = self.pipes.get_mut(&id) {
			pipe.ready(event_loop, events);
		}
	}

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message) {
		for (_, pipe) in self.pipes.iter_mut() {
			if pipe.is_ready() {
				pipe.send(msg);
				return;
			}
		}
	}
}
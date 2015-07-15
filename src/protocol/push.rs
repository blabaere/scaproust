use std::collections::HashMap;
use std::io;
use mio;

use super::Protocol;
use pipe::Pipe as Pipe;
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

	fn remove_pipe(&mut self, id: usize) -> Option<String> {
		self.pipes.remove(&id).map(|p| p.addr())
	}

	fn ready(&mut self, event_loop: &mut EventLoop, id: usize, events: mio::EventSet) -> io::Result<()> {
		if let Some(pipe) = self.pipes.get_mut(&id) {
			pipe.ready(event_loop, events)
		} else {
			Ok(())
		}
	}

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message) {
		// Something's wrong here in the way it is done.
		// The message should be shared between each pipe.
		// The first pipe to be write-ready should take it (like in Option.take)
		// This pipe then starts sending it, notifying upstream success or failure

		// Since this behavior is protocol specific, it can not be implemented at the pipe level.
		// So there has to be an additional layer above pipe, maybe inside Push

		// The send operation progress could be monitored inside the pipe
		for (_, pipe) in self.pipes.iter_mut() {
			if pipe.is_ready() {
				pipe.send(msg);
				return;
			}
		}
	}
}
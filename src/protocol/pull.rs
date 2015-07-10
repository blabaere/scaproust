use mio;

use super::Protocol;
use pipe::Pipe as Pipe;
use global::SocketType as SocketType;
use EventLoop;
use Message;

#[derive(Debug, Copy, Clone)]
pub struct Pull;

impl Protocol for Pull {
	fn id(&self) -> u16 {
		SocketType::Pull.id()
	}

	fn peer_id(&self) -> u16 {
		SocketType::Push.id()
	}

	fn add_pipe(&mut self, id: usize, _: Pipe) {
	}

	fn ready(&mut self, event_loop: &mut EventLoop, id: usize, events: mio::EventSet) {

	}

	fn send(&mut self, event_loop: &mut EventLoop, msg: Message) {

	}
}
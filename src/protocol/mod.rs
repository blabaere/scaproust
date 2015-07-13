use std::io;

use mio;

use global::SocketType as SocketType;
use pipe::Pipe as Pipe;
use EventLoop;
use Message;

pub mod push;
pub mod pull;

pub trait Protocol {
	fn id(&self) -> u16;
	fn peer_id(&self) -> u16;

	fn add_pipe(&mut self, id: usize, pipe: Pipe);

	fn ready(&mut self, event_loop: &mut EventLoop, id: usize, events: mio::EventSet) -> io::Result<()>;
	fn send(&mut self, event_loop: &mut EventLoop, msg: Message);
}


pub fn create_protocol(socket_type: SocketType) -> Box<Protocol> {
	match socket_type {
		SocketType::Push => Box::new(push::Push::new()),
		SocketType::Pull => Box::new(pull::Pull),
		_ => panic!("")
	}
}

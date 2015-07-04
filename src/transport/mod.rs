use std::io;
use mio;

pub mod tcp;
//pub mod ipc;

use EventLoop;

pub trait Transport {
	fn connector(&self, addr: &str, id: usize) -> Box<Connector>;
}

pub trait Connector {
	fn connect(&self, event_loop: &mut EventLoop) -> Result<Box<Connection>, io::Error>;
}

pub trait Connection {
	fn as_evented(&self) -> &mio::Evented;
	fn readable(&mut self, event_loop: &mut EventLoop, hint: mio::ReadHint) -> io::Result<bool>;
	fn writable(&mut self, event_loop: &mut EventLoop) -> io::Result<bool>;
}

pub fn create_transport(addr: &str) -> Box<Transport> {
	Box::new(tcp::Tcp)
}
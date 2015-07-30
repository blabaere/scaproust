use std::io;
use mio;

pub mod tcp;
//pub mod ipc;

// represents the transport media 
pub trait Transport {
	fn connect(&self, addr: &str) -> io::Result<Box<Connection>>;
	fn listen(&self, addr: &str) -> io::Result<Box<Acceptor>>;
}

// represents a connection in a given media
// only needs to expose mio compatible features:
// - an exposed id to be used an mio token
// - transfert bytes in non-blocking manner
// - being registrable into the event loop
pub trait Connection {
	//fn id(&self) -> usize;
	fn as_evented(&self) -> &mio::Evented;
	fn try_read(&mut self, buf: &mut [u8]) -> Result<Option<usize>, io::Error>;
	fn try_write(&mut self, buf: &[u8]) -> Result<Option<usize>, io::Error>;
}

pub trait Acceptor {
	//fn id(&self) -> usize;
	fn accept(&mut self) -> io::Result<Box<Connection>>;
}

pub fn create_transport(_: &str) -> Box<Transport> {
	Box::new(tcp::Tcp)
}

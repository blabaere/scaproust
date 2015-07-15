use std::io;
use mio;

pub mod tcp;
//pub mod ipc;

// represents the transport media 
pub trait Transport {
	fn connect(&self, addr: &str) -> Result<Box<Connection>, io::Error>;
}

// represents a connection in a given media
// only needs to expose mio compatible features
// - transfert bytes in non-blocking manner
// - being registrable into the event loop
pub trait Connection {
	fn as_evented(&self) -> &mio::Evented;
	fn try_read(&mut self, buf: &mut [u8]) -> Result<Option<usize>, io::Error>;
	fn try_write(&mut self, buf: &[u8]) -> Result<Option<usize>, io::Error>;
}

pub fn create_transport(_: &str) -> Box<Transport> {
	Box::new(tcp::Tcp)
}

pub mod tcp;
//pub mod ipc;

pub trait Transport {
	fn connector(&self, addr: &str) -> Box<Connector>;
}

pub trait Connector {
	fn connect(&self) -> Box<Connection>;
}

pub trait Connection {
	// Add code here
}

pub fn create_transport(addr: &str) -> Box<Transport> {
	Box::new(tcp::Tcp)
}
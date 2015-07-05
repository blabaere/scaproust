use global::SocketType as SocketType;

pub mod push;
pub mod pull;

pub trait Protocol {
	fn id(&self) -> u16;
	fn peer_id(&self) -> u16;
}

pub fn create_protocol(socket_type: SocketType) -> Box<Protocol> {
	match socket_type {
		SocketType::Push => Box::new(push::Push),
		SocketType::Pull => Box::new(pull::Pull),
		_ => panic!("")
	}
}
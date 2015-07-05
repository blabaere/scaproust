use super::Protocol;
use global::SocketType as SocketType;
#[derive(Debug, Copy, Clone)]
pub struct Push;

impl Protocol for Push {
	fn id(&self) -> u16 {
		SocketType::Push.id()
	}

	fn peer_id(&self) -> u16 {
		SocketType::Pull.id()
	}
	
}
use super::Protocol;
use global::SocketType as SocketType;
#[derive(Debug, Copy, Clone)]
pub struct Pull;

impl Protocol for Pull {
	fn id(&self) -> u16 {
		SocketType::Pull.id()
	}

	fn peer_id(&self) -> u16 {
		SocketType::Push.id()
	}
	
}
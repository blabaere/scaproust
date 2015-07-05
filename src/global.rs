
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SocketType {
    Pair       = (1 * 16),
	Pub        = (2 * 16),
	Sub        = (2 * 16) + 1,
	Req        = (3 * 16),
	Rep        = (3 * 16) + 1,
	Push       = (5 * 16),
	Pull       = (5 * 16) + 1,
	Surveyor   = (6 * 16),
	Respondent = (6 * 16) + 1,
	Bus        = (7 * 16)
}

impl SocketType {
	pub fn id(&self) -> u16 {
		*self as u16
	}
}

use std::sync::mpsc;

pub enum EventLoopCmd {
	Ping,
	CreateSocket,
	PingSocket(usize),
	Shutdown
}

pub enum SessionEvt {
	Pong,
	SocketCreated(usize, mpsc::Receiver<SocketEvt>),
	SocketNotCreated
}

pub enum SocketEvt {
    Pong
}

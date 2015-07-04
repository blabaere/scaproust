use std::sync::mpsc;
use std::io;

use global::SocketType as SocketType;

pub enum EventLoopCmd {
	Ping,
	CreateSocket(SocketType),
	PingSocket(usize),
	ConnectSocket(usize, String),
	Shutdown
}

pub enum EventLoopTimeout {
	Reconnect(usize, String)
}

pub enum SessionEvt {
	Pong,
	SocketCreated(usize, mpsc::Receiver<SocketEvt>),
	SocketNotCreated
}

pub enum SocketEvt {
    Pong,
    Connected,
    NotConnected(io::Error)
}

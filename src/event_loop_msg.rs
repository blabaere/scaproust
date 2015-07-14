use std::sync::mpsc;
use std::io;

use global::SocketType as SocketType;
use Message;

pub enum EventLoopCmd {
	SessionLevel(SessionCmd),
	SocketLevel(usize, SocketCmd)
}

pub enum SessionCmd {
	Ping,
	CreateSocket(SocketType),
	Shutdown
}

pub enum SocketCmd {
	Ping,
	Connect(String),
	SendMsg(Message)
}

pub enum EventLoopTimeout {
	Reconnect(usize, String)
}

pub enum SessionEvt {
	Pong,
	SocketCreated(usize, mpsc::Receiver<SocketEvt>),
	SocketNotCreated}

pub enum SocketEvt {
    Pong,
    Connected,
    NotConnected(io::Error),
	MsgSent,
	MsgNotSent(io::Error)
}

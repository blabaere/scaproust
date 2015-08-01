use std::sync::mpsc;
use std::io;

use mio;

use global::*;
use Message;

pub enum EventLoopCmd {
	SessionLevel(SessionCmd),
	SocketLevel(SocketId, SocketCmd)
}

pub enum SessionCmd {
	Ping,
	CreateSocket(SocketType),
	Shutdown
}

pub enum SocketCmd {
	Ping,
	Connect(String),
	Bind(String),
	SendMsg(Message)
}

pub enum EventLoopTimeout {
	Reconnect(mio::Token, String),
	//Rebind(mio::Token, String)
}

pub enum SessionEvt {
	Pong,
	SocketCreated(SocketId, mpsc::Receiver<SocketEvt>),
	SocketNotCreated}

pub enum SocketEvt {
    Pong,
    Connected,
    NotConnected(io::Error),
    Bound,
    NotBound(io::Error),
	MsgSent,
	MsgNotSent(io::Error)
}

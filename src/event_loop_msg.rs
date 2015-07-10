use std::sync::mpsc;
use std::io;

use global::SocketType as SocketType;
use Message;

pub enum EventLoopCmd {
	Ping,
	CreateSocket(SocketType),
	PingSocket(usize),
	ConnectSocket(usize, String),
	SendMsg(usize, Message),
	Shutdown
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

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
	CreateSocket(SocketType),
	Shutdown
}

pub enum SocketCmd {
	Connect(String),
	Bind(String),
	SendMsg(Message),
	RecvMsg
}

pub enum EventLoopTimeout {
	Reconnect(mio::Token, String),
	Rebind(mio::Token, String),
	CancelSend(SocketId),
	CancelRecv(SocketId)
}

pub enum SessionEvt {
	SocketCreated(SocketId, mpsc::Receiver<SocketEvt>),
}

pub enum SocketEvt {
    Connected,
    NotConnected(io::Error),
    Bound,
    NotBound(io::Error),
	MsgSent,
	MsgNotSent(io::Error),
	MsgRecv(Message),
	MsgNotRecv(io::Error)
}

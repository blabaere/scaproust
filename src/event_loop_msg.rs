// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::mpsc;
use std::io;
use std::time;

use mio;

use global::*;
use Message;

/// information sent through the event loop channel so components can communicate with each others.
pub enum EventLoopSignal {
    Cmd(CmdSignal),
    Evt(EvtSignal)
}

pub enum CmdSignal {
    Session(SessionCmdSignal),
    Socket(SocketId, SocketCmdSignal)
}

pub enum SessionCmdSignal {
    CreateSocket(SocketType),
    Shutdown
}

pub enum SocketCmdSignal {
    Connect(String),
    Bind(String),
    SendMsg(Message),
    RecvMsg,
    SetOption(SocketOption)
}

pub enum SocketOption {
    SendTimeout(time::Duration),
    RecvTimeout(time::Duration),
    Subscribe(String),
    Unsubscribe(String),
    SurveyDeadline(time::Duration),
    ResendInterval(time::Duration)
}

// it is kind of ugly
// both events are related to a pipe
// maybe the socket id and the pipe token
// should be exposed in the same way ?
// maybe not, who knows ...
pub enum EvtSignal {
    Socket(SocketId, SocketEvtSignal),
    Pipe(mio::Token, PipeEvtSignal)
}

pub enum SocketEvtSignal {
    Connected(mio::Token),
    Bound(mio::Token)
}

pub enum PipeEvtSignal {
    Opened,
    MsgRcv(Message),
    MsgSnd
}

pub enum EventLoopTimeout {
    Reconnect(mio::Token, String),
    Rebind(mio::Token, String),
    CancelSend(SocketId),
    CancelRecv(SocketId),
    CancelSurvey(SocketId),
    CancelResend(SocketId)
}

pub enum SessionNotify {
    SocketCreated(SocketId, mpsc::Receiver<SocketNotify>)
}

pub enum SocketNotify {
    Connected,
    NotConnected(io::Error),
    Bound,
    NotBound(io::Error),
    MsgSent,
    MsgNotSent(io::Error),
    MsgRecv(Message),
    MsgNotRecv(io::Error),
    OptionSet,
    OptionNotSet(io::Error)
}

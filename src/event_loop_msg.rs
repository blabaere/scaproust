// Copyright (c) 2016 Benoît Labaere (benoit.labaere@gmail.com)
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

impl EventLoopSignal {
    pub fn name(&self) -> &'static str {
        match *self {
            EventLoopSignal::Cmd(_) => "Cmd",
            EventLoopSignal::Evt(_) => "Evt"
        }
    }
}

pub enum CmdSignal {
    Session(SessionCmdSignal),
    Socket(SocketId, SocketCmdSignal)
}

impl CmdSignal {
    pub fn name(&self) -> &'static str {
        match *self {
            CmdSignal::Session(_)  => "Session",
            CmdSignal::Socket(_,_) => "Socket"
        }
    }
}

pub enum SessionCmdSignal {
    CreateSocket(SocketType),
    Shutdown
}

impl SessionCmdSignal {
    pub fn name(&self) -> &'static str {
        match *self {
            SessionCmdSignal::CreateSocket(_) => "CreateSocket",
            SessionCmdSignal::Shutdown        => "Shutdown"
        }
    }
}

pub enum SocketCmdSignal {
    Connect(String),
    Bind(String),
    SendMsg(Message),
    RecvMsg,
    SetOption(SocketOption)
}

impl SocketCmdSignal {
    pub fn name(&self) -> &'static str {
        match *self {
            SocketCmdSignal::Connect(_)   => "Connect",
            SocketCmdSignal::Bind(_)      => "Bind",
            SocketCmdSignal::SendMsg(_)   => "SendMsg",
            SocketCmdSignal::RecvMsg      => "RecvMsg",
            SocketCmdSignal::SetOption(_) => "SetOption"
        }
    }
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

impl EvtSignal {
    pub fn name(&self) -> &'static str {
        match *self {
            EvtSignal::Socket(_,_) => "Socket",
            EvtSignal::Pipe(_,_)   => "Pipe"
        }
    }
}

pub enum SocketEvtSignal {
    Connected(mio::Token),
    Bound(mio::Token)
}

impl SocketEvtSignal {
    pub fn name(&self) -> &'static str {
        match *self {
            SocketEvtSignal::Connected(_) => "Connected",
            SocketEvtSignal::Bound(_)     => "Bound"
        }
    }
}

pub enum PipeEvtSignal {
    Opened,
    MsgRcv(Message),
    MsgSnd
}

impl PipeEvtSignal {
    pub fn name(&self) -> &'static str {
        match *self {
            PipeEvtSignal::Opened    => "Opened",
            PipeEvtSignal::MsgRcv(_) => "MsgRcv",
            PipeEvtSignal::MsgSnd    => "MsgSnd"
        }
    }
}

pub enum EventLoopTimeout {
    Reconnect(mio::Token, String),
    Rebind(mio::Token, String),
    CancelSend(SocketId),
    CancelRecv(SocketId),
    CancelSurvey(SocketId),
    Resend(SocketId)
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

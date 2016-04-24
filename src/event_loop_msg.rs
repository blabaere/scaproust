// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::sync::mpsc;
use std::io;
use std::time;

use mio;

use global::*;
use Message;

/// Message flowing through the event loop channel so components can communicate with each others.
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

/// Commands sent by facade components to *backend* components
pub enum CmdSignal {
    Session(SessionCmdSignal),
    Socket(SocketId, SocketCmdSignal),
    Probe(ProbeId, ProbeCmdSignal)
}

impl CmdSignal {
    pub fn name(&self) -> &'static str {
        match *self {
            CmdSignal::Session(_) => "Session",
            CmdSignal::Socket(_, _) => "Socket",
            CmdSignal::Probe(_, _) => "Probe"
        }
    }
}

/// Commands sent to the session
pub enum SessionCmdSignal {
    CreateSocket(SocketType),
    DestroySocket(SocketId),
    CreateProbe(SocketId, SocketId),
    DestroyProbe(ProbeId),
    Shutdown
}

impl SessionCmdSignal {
    pub fn name(&self) -> &'static str {
        match *self {
            SessionCmdSignal::CreateSocket(_) => "CreateSocket",
            SessionCmdSignal::DestroySocket(_) => "DestroySocket",
            SessionCmdSignal::CreateProbe(_, _) => "CreateProbe",
            SessionCmdSignal::DestroyProbe(_) => "DestroyProbe",
            SessionCmdSignal::Shutdown => "Shutdown"
        }
    }
}

/// Commands sent to a socket
pub enum SocketCmdSignal {
    Connect(String),
    Bind(String),
    SendMsg(Message),
    RecvMsg,
    SetOption(SocketOption),
    Shutdown(mio::Token)
}

impl SocketCmdSignal {
    pub fn name(&self) -> &'static str {
        match *self {
            SocketCmdSignal::Connect(_) => "Connect",
            SocketCmdSignal::Bind(_) => "Bind",
            SocketCmdSignal::SendMsg(_) => "SendMsg",
            SocketCmdSignal::RecvMsg => "RecvMsg",
            SocketCmdSignal::SetOption(_) => "SetOption",
            SocketCmdSignal::Shutdown(_) => "Shutdown"
        }
    }
}

pub enum SocketOption {
    #[doc(hidden)]
    DeviceItem(bool),

    /// See [Socket::set_send_timeout](struct.Socket.html#method.set_send_timeout).
    SendTimeout(time::Duration),

    /// See [Socket::set_recv_timeout](struct.Socket.html#method.set_recv_timeout).
    RecvTimeout(time::Duration),

    /// See [Socket::set_send_priority](struct.Socket.html#method.set_send_priority).
    SendPriority(u8),

    /// See [Socket::set_recv_priority](struct.Socket.html#method.set_recv_priority).
    RecvPriority(u8),

    /// For connection-based transports such as TCP, this option specifies how long to wait, 
    /// when connection is broken before trying to re-establish it. 
    /// Note that actual reconnect interval may be randomised to some extent 
    /// to prevent severe reconnection storms. Default value is 0.1 second.
    ReconnectInterval(time::Duration),

    /// This option is to be used only in addition to ReconnectInterval option.
    /// It specifies maximum reconnection interval. On each reconnect attempt,
    /// the previous interval is doubled until ReconnectIntervalMax is reached.
    /// Value of 0 means that no exponential backoff is performed and reconnect interval is based only on ReconnectInterval.
    /// If ReconnectIntervalMax is less than ReconnectInterval, it is ignored. 
    /// Default value is 0.
    ReconnectIntervalMax(time::Duration),

    /// Defined on `Sub` socket. Subscribes for a particular topic.
    /// A single `Sub` socket can handle multiple subscriptions.
    Subscribe(String),

    /// Defined on Sub` socket. Unsubscribes from a particular topic.
    Unsubscribe(String),

    SurveyDeadline(time::Duration),
    ResendInterval(time::Duration),

    /// See [Socket::set_tcp_nodelay](struct.Socket.html#method.set_tcp_nodelay).
    TcpNoDelay(bool)
}

/// Commands sent to a probe
pub enum ProbeCmdSignal {
    PollReadable
}

impl ProbeCmdSignal {
    pub fn name(&self) -> &'static str {
        match *self {
            ProbeCmdSignal::PollReadable => "PollReadable"
        }
    }
}

/// Events raised by components living in the event loop, resulting from the execution of commands.
pub enum EvtSignal {
    Socket(SocketId, SocketEvtSignal),
    Pipe(mio::Token, PipeEvtSignal)
}

impl EvtSignal {
    pub fn name(&self) -> &'static str {
        match *self {
            EvtSignal::Socket(_, _) => "Socket",
            EvtSignal::Pipe(_, _) => "Pipe"
        }
    }
}

// Events raised by sockets
pub enum SocketEvtSignal {
    PipeAdded(mio::Token),
    AcceptorAdded(mio::Token),
    Readable
}

impl SocketEvtSignal {
    pub fn name(&self) -> &'static str {
        match *self {
            SocketEvtSignal::PipeAdded(_) => "PipeAdded",
            SocketEvtSignal::AcceptorAdded(_) => "AcceptorAdded",
            SocketEvtSignal::Readable => "Readable"
        }
    }
}

/// Events raised by pipes
pub enum PipeEvtSignal {
    Opened,
    Closed,
    Error(u32),
    RecvDone(Message),
    RecvBlocked,
    SendDone,
    SendBlocked
}

impl PipeEvtSignal {
    pub fn name(&self) -> &'static str {
        match *self {
            PipeEvtSignal::Opened => "Opened",
            PipeEvtSignal::Closed => "Closed",
            PipeEvtSignal::Error(_) => "Error",
            PipeEvtSignal::RecvDone(_) => "RecvDone",
            PipeEvtSignal::RecvBlocked => "RecvBlocked",
            PipeEvtSignal::SendDone => "SendDone",
            PipeEvtSignal::SendBlocked => "SendBlocked"
        }
    }
}

/// Events raised by a timer
pub enum EventLoopTimeout {
    Reconnect(SocketId, mio::Token, String, u32),
    Rebind(SocketId, mio::Token, String, u32),
    CancelSend(SocketId),
    CancelRecv(SocketId),
    CancelSurvey(SocketId),
    Resend(SocketId)
}

/// Notifications sent by the *backend* session as reply to the commands sent by the facade session.
pub enum SessionNotify {
    SocketCreated(SocketId, mpsc::Receiver<SocketNotify>),
    ProbeCreated(ProbeId, mpsc::Receiver<ProbeNotify>),
    ProbeNotCreated(io::Error),
    Shutdown
}

/// Notifications sent by the *backend* socket as reply to the commands sent by the facade socket.
pub enum SocketNotify {
    Connected(mio::Token),
    NotConnected(io::Error),
    Bound(mio::Token),
    NotBound(io::Error),
    MsgSent,
    MsgNotSent(io::Error),
    MsgRecv(Message),
    MsgNotRecv(io::Error),
    OptionSet,
    OptionNotSet(io::Error)
}

/// Notifications sent by the probe as reply to the commands sent by the facade device.
pub enum ProbeNotify {
    Ok(bool, bool)
}

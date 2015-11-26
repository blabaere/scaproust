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

pub enum EventLoopSignal {
    Session(SessionSignal),
    Socket(SocketId, SocketSignal)
}

pub enum SessionSignal {
    CreateSocket(SocketType),
    Shutdown
}

pub enum SocketSignal {
    Connect(String),
    Connected(mio::Token),
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

pub enum EventLoopTimeout {
    Reconnect(mio::Token, String),
    Rebind(mio::Token, String),
    CancelSend(SocketId),
    CancelRecv(SocketId),
    CancelSurvey(SocketId),
    CancelResend(SocketId)
}

pub enum SessionEvt {
    SocketCreated(SocketId, mpsc::Receiver<SocketEvt>)
}

pub enum SocketEvt {
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

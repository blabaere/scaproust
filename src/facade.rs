// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;
use std::thread;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::time;

use mio;
use mio::Sender;

use global::*;
use event_loop_msg::*;

use Message;
use EventLoop;
use session::Session;

pub struct SessionFacade {
    cmd_sender: mio::Sender<EventLoopSignal>,
    evt_receiver: mpsc::Receiver<SessionNotify>
}

impl SessionFacade {
    pub fn new() -> io::Result<SessionFacade> {
        let mut builder = mio::EventLoopBuilder::new();

        builder.
            notify_capacity(4_096).
            messages_per_tick(256).
            timer_tick(time::Duration::from_millis(15)).
            timer_wheel_size(1_024).
            timer_capacity(4_096);

        let mut event_loop = try!(builder.build());
        let (tx, rx) = mpsc::channel();
        let session = SessionFacade { 
            cmd_sender: event_loop.channel(),
            evt_receiver: rx };

        thread::spawn(move || SessionFacade::run_event_loop(&mut event_loop, tx));

        Ok(session)
    }

    fn run_event_loop(event_loop: &mut EventLoop, evt_tx: mpsc::Sender<SessionNotify>) {
        let mut handler = Session::new(evt_tx);
        let exec = event_loop.run(&mut handler);

        match exec {
            Ok(_) => debug!("event loop exited"),
            Err(e) => error!("event loop failed to run: {}", e)
        }
    }

    fn send_cmd(&self, cmd: SessionCmdSignal) -> Result<(), io::Error> {
        let cmd_sig = CmdSignal::Session(cmd);
        let loop_sig = EventLoopSignal::Cmd(cmd_sig);

        self.cmd_sender.send(loop_sig).map_err(|e| convert_notify_err(e))
    }

    pub fn create_socket(&self, socket_type: SocketType) -> io::Result<SocketFacade> {
        let cmd = SessionCmdSignal::CreateSocket(socket_type);

        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SessionNotify::SocketCreated(id, rx)) => Ok(self.new_socket(id, socket_type, rx)),
            Err(_)                                   => Err(other_io_error("evt channel closed"))
        }
    }

    fn new_socket(&self, id: SocketId, socket_type: SocketType, rx: mpsc::Receiver<SocketNotify>) -> SocketFacade {
        SocketFacade::new(id, socket_type, self.cmd_sender.clone(), rx)
    }
}

impl Drop for SessionFacade {
    fn drop(&mut self) {
        let _ = self.send_cmd(SessionCmdSignal::Shutdown);
    }
}

pub struct SocketFacade {
    id: SocketId,
    socket_type: SocketType, 
    cmd_sender: Sender<EventLoopSignal>,
    evt_receiver: Receiver<SocketNotify>
    // Could use https://github.com/polyfractal/bounded-spsc-queue ?
    // Maybe once a smart waiting strategy is available (like spin, then sleep 0, then sleep 1, then mutex ?)
    // or something that would help for poll
}

impl SocketFacade {
    pub fn new(
        id: SocketId,
        socket_type: SocketType, 
        cmd_tx: Sender<EventLoopSignal>, 
        evt_rx: Receiver<SocketNotify>) -> SocketFacade {
        SocketFacade { 
            id: id, 
            socket_type: socket_type,
            cmd_sender: cmd_tx, 
            evt_receiver: evt_rx 
        }
    }

    pub fn get_socket_type(&self) -> SocketType {
        self.socket_type
    }

    fn send_cmd(&self, cmd: SocketCmdSignal) -> Result<(), io::Error> {
        let cmd_sig = CmdSignal::Socket(self.id, cmd);
        let loop_sig = EventLoopSignal::Cmd(cmd_sig);

        self.cmd_sender.send(loop_sig).map_err(|e| convert_notify_err(e))
    }

    pub fn connect(&mut self, addr: &str) -> Result<(), io::Error> {
        let cmd = SocketCmdSignal::Connect(addr.to_owned());
        
        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SocketNotify::Connected)       => Ok(()),
            Ok(SocketNotify::NotConnected(e)) => Err(e),
            Ok(_)                             => Err(other_io_error("unexpected evt")),
            Err(_)                            => Err(other_io_error("evt channel closed"))
        }
    }

    pub fn bind(&mut self, addr: &str) -> Result<(), io::Error> {
        let cmd = SocketCmdSignal::Bind(addr.to_owned());
        
        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SocketNotify::Bound)       => Ok(()),
            Ok(SocketNotify::NotBound(e)) => Err(e),
            Ok(_)                      => Err(other_io_error("unexpected evt")),
            Err(_)                     => Err(other_io_error("evt channel closed"))
        }
    }

    pub fn send(&mut self, buffer: Vec<u8>) -> Result<(), io::Error> {
        self.send_msg(Message::with_body(buffer))
    }

    pub fn send_msg(&mut self, msg: Message) -> Result<(), io::Error> {
        let cmd = SocketCmdSignal::SendMsg(msg);

        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SocketNotify::MsgSent)       => Ok(()),
            Ok(SocketNotify::MsgNotSent(e)) => Err(e),
            Ok(_)                           => Err(other_io_error("unexpected evt")),
            Err(_)                          => Err(other_io_error("evt channel closed"))
        }
    }

    pub fn recv(&mut self) -> Result<Vec<u8>, io::Error> {
        self.recv_msg().map(|msg| msg.to_buffer())
    }

    pub fn recv_msg(&mut self) -> Result<Message, io::Error> {
        let cmd = SocketCmdSignal::RecvMsg;

        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SocketNotify::MsgRecv(msg))  => Ok(msg),
            Ok(SocketNotify::MsgNotRecv(e)) => Err(e),
            Ok(_)                           => Err(other_io_error("unexpected evt")),
            Err(_)                          => Err(other_io_error("evt channel closed"))
        }
    }

    pub fn set_option(&mut self, option: SocketOption) -> io::Result<()> {
        let cmd = SocketCmdSignal::SetOption(option);

        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SocketNotify::OptionSet)       => Ok(()),
            Ok(SocketNotify::OptionNotSet(e)) => Err(e),
            Ok(_)                             => Err(other_io_error("unexpected evt")),
            Err(_)                            => Err(other_io_error("evt channel closed"))
        }
    }

    pub fn set_send_timeout(&mut self, timeout: time::Duration) -> io::Result<()> {
        self.set_option(SocketOption::SendTimeout(timeout))
    }

    pub fn set_recv_timeout(&mut self, timeout: time::Duration) -> io::Result<()> {
        self.set_option(SocketOption::RecvTimeout(timeout))
    }

    pub fn run_relay_device(mut self) -> io::Result<()> {
        loop {
            let msg = try!(self.recv_msg());

            try!(self.send_msg(msg));
        }
    }

    pub fn run_bridge_device(self, other: SocketFacade) -> io::Result<()> {
        if !self.socket_type.matches(other.socket_type) {
            other_io_error("Socket types do not match");
        }

        match self.get_socket_type() {
            SocketType::Pull | SocketType::Sub => self.one_way_device(other),
            SocketType::Push | SocketType::Pub => other.one_way_device(self),
            SocketType::Req        |
            SocketType::Rep        |
            SocketType::Respondent |
            SocketType::Surveyor   |
            SocketType::Pair       => self.two_way_device(other),
            SocketType::Bus        => Err(invalid_data_io_error("socket type not supported"))
        }
    }

    fn one_way_device(mut self: SocketFacade, mut sender: SocketFacade) -> io::Result<()> {
        loop {
            let msg = try!(self.recv_msg());

            try!(sender.send_msg(msg));
        }
    }

    fn two_way_device(mut self: SocketFacade, mut other: SocketFacade) -> io::Result<()> {
        Err(other_io_error("not implemented"))
    }
}

#[cfg(test)]
mod tests {
    use super::SessionFacade;
    use global::SocketType;

    #[test]
    fn session_can_create_a_socket() {
        let session = SessionFacade::new().unwrap();
        let socket = session.create_socket(SocketType::Push).unwrap();

        drop(socket);
    }

    #[test]
    fn can_connect_socket() {
        let session = SessionFacade::new().unwrap();
        let mut socket = session.create_socket(SocketType::Pair).unwrap();

        socket.connect("tcp://127.0.0.1:5454").unwrap();
    }

    #[test]
    fn can_try_connect_socket() {
        let session = SessionFacade::new().unwrap();
        let mut socket = session.create_socket(SocketType::Push).unwrap();

        assert!(socket.connect("tcp://this should not work").is_err());
    }
}
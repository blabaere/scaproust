// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
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
    cmd_sender: mio::Sender<EventLoopCmd>,
    evt_receiver: mpsc::Receiver<SessionEvt>
}

impl SessionFacade {
    pub fn new() -> io::Result<SessionFacade> {
        let mut config = mio::EventLoopConfig::new();

        config.
            io_poll_timeout_ms(250).
            notify_capacity(4_096).
            messages_per_tick(256).
            timer_tick_ms(15).
            timer_wheel_size(1_024).
            timer_capacity(4_096);

        let mut event_loop = try!(mio::EventLoop::configured(config));
        let (tx, rx) = mpsc::channel();
        let session = SessionFacade { 
            cmd_sender: event_loop.channel(),
            evt_receiver: rx };

        thread::spawn(move || SessionFacade::run_event_loop(&mut event_loop, tx));

        Ok(session)
    }

    fn run_event_loop(event_loop: &mut EventLoop, evt_tx: mpsc::Sender<SessionEvt>) {
        let mut handler = Session::new(evt_tx);
        let exec = event_loop.run(&mut handler);

        match exec {
            Ok(_) => debug!("event loop exited"),
            Err(e) => error!("event loop failed to run: {}", e)
        }
    }

    fn send_cmd(&self, session_cmd: SessionCmd) -> Result<(), io::Error> {
        let cmd = EventLoopCmd::SessionLevel(session_cmd);

        self.cmd_sender.send(cmd).map_err(|e| convert_notify_err(e))
    }

    pub fn create_socket(&self, socket_type: SocketType) -> io::Result<SocketFacade> {
        let session_cmd = SessionCmd::CreateSocket(socket_type);

        try!(self.send_cmd(session_cmd));

        match self.evt_receiver.recv() {
            Ok(SessionEvt::SocketCreated(id, rx)) => Ok(self.new_socket(id, rx)),
            Err(_)                                => Err(other_io_error("evt channel closed"))
        }
    }

    fn new_socket(&self, id: SocketId, rx: mpsc::Receiver<SocketEvt>) -> SocketFacade {
        SocketFacade::new(id, self.cmd_sender.clone(), rx)
    }
}

impl Drop for SessionFacade {
    fn drop(&mut self) {
        let session_cmd = SessionCmd::Shutdown;
        let cmd = EventLoopCmd::SessionLevel(session_cmd);
        
        let _ = self.cmd_sender.send(cmd);
    }
}

pub struct SocketFacade {
    id: SocketId,
    cmd_sender: Sender<EventLoopCmd>,
    evt_receiver: Receiver<SocketEvt>
    // Could use https://github.com/polyfractal/bounded-spsc-queue ?
    // Maybe once a smart waiting strategy is available (like spin, then sleep 0, then sleep 1, then mutex ?)
}

impl SocketFacade {
    pub fn new(id: SocketId, cmd_tx: Sender<EventLoopCmd>, evt_rx: Receiver<SocketEvt>) -> SocketFacade {
        SocketFacade { id: id, cmd_sender: cmd_tx, evt_receiver: evt_rx }
    }

    fn send_cmd(&self, socket_cmd: SocketCmd) -> Result<(), io::Error> {
        let cmd = EventLoopCmd::SocketLevel(self.id, socket_cmd);

        self.cmd_sender.send(cmd).map_err(|e| convert_notify_err(e))
    }

    pub fn connect(&mut self, addr: &str) -> Result<(), io::Error> {
        let cmd = SocketCmd::Connect(addr.to_owned());
        
        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SocketEvt::Connected)       => Ok(()),
            Ok(SocketEvt::NotConnected(e)) => Err(e),
            Ok(_)                          => Err(other_io_error("unexpected evt")),
            Err(_)                         => Err(other_io_error("evt channel closed"))
        }
    }

    pub fn bind(&mut self, addr: &str) -> Result<(), io::Error> {
        let cmd = SocketCmd::Bind(addr.to_owned());
        
        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SocketEvt::Bound)       => Ok(()),
            Ok(SocketEvt::NotBound(e)) => Err(e),
            Ok(_)                      => Err(other_io_error("unexpected evt")),
            Err(_)                     => Err(other_io_error("evt channel closed"))
        }
    }

    pub fn send(&mut self, buffer: Vec<u8>) -> Result<(), io::Error> {
        self.send_msg(Message::with_body(buffer))
    }

    pub fn send_msg(&mut self, msg: Message) -> Result<(), io::Error> {
        let cmd = SocketCmd::SendMsg(msg);

        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SocketEvt::MsgSent)       => Ok(()),
            Ok(SocketEvt::MsgNotSent(e)) => Err(e),
            Ok(_)                        => Err(other_io_error("unexpected evt")),
            Err(_)                       => Err(other_io_error("evt channel closed"))
        }
    }

    pub fn recv(&mut self) -> Result<Vec<u8>, io::Error> {
        self.recv_msg().map(|msg| msg.to_buffer())
    }

    pub fn recv_msg(&mut self) -> Result<Message, io::Error> {
        try!(self.send_cmd(SocketCmd::RecvMsg));

        match self.evt_receiver.recv() {
            Ok(SocketEvt::MsgRecv(msg))  => Ok(msg),
            Ok(SocketEvt::MsgNotRecv(e)) => Err(e),
            Ok(_)                        => Err(other_io_error("unexpected evt")),
            Err(_)                       => Err(other_io_error("evt channel closed"))
        }
    }

    fn set_option(&mut self, option: SocketOption) -> io::Result<()> {
        let cmd = SocketCmd::SetOption(option);

        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SocketEvt::OptionSet)       => Ok(()),
            Ok(SocketEvt::OptionNotSet(e)) => Err(e),
            Ok(_)                          => Err(other_io_error("unexpected evt")),
            Err(_)                         => Err(other_io_error("evt channel closed"))
        }
    }

    pub fn set_send_timeout(&mut self, timeout: time::Duration) -> io::Result<()> {
        self.set_option(SocketOption::SendTimeout(timeout))
    }

    pub fn set_recv_timeout(&mut self, timeout: time::Duration) -> io::Result<()> {
        self.set_option(SocketOption::RecvTimeout(timeout))
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
        let mut socket = session.create_socket(SocketType::Push).unwrap();

        assert!(socket.connect("tcp://127.0.0.1:5454").is_ok());
    }

    #[test]
    fn can_try_connect_socket() {
        let session = SessionFacade::new().unwrap();
        let mut socket = session.create_socket(SocketType::Push).unwrap();

        assert!(socket.connect("tcp://this should not work").is_err());
    }
}
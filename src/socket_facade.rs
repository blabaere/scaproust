// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::time;

use mio::Sender;

use global::*;
use event_loop_msg::*;
use Message;

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
            try!(self.recv_msg().and_then(|msg| self.send_msg(msg)));
        }
    }

    pub fn run_bridge_device(self, other: SocketFacade) -> io::Result<()> {
        if !self.socket_type.matches(other.socket_type) {
            other_io_error("Socket types do not match");
        }

        match self.get_socket_type() {
            SocketType::Pull | SocketType::Sub => self.run_one_way_device(other),
            SocketType::Push | SocketType::Pub => other.run_one_way_device(self),
            SocketType::Req        |
            SocketType::Rep        |
            SocketType::Respondent |
            SocketType::Surveyor   |
            SocketType::Bus        |
            SocketType::Pair       => self.run_two_way_device(other)
        }
    }

    fn run_one_way_device(mut self, mut other: SocketFacade) -> io::Result<()> {
        loop {
            try!(self.forward_msg(&mut other))
        }
    }

    fn run_two_way_device(mut self, mut other: SocketFacade) -> io::Result<()> {
        let probe = try!(self.setup_two_way_device(&other));

        loop {
            match probe.recv() {
                Ok(PollResult(votes)) => try!(self.on_two_way_device_step(&mut other, votes)),
                Err(_)                => return Err(other_io_error("device evt channel closed"))
            }
        }
    }

    fn setup_two_way_device(&self, other: &SocketFacade) -> io::Result<mpsc::Receiver<PollResult>> {
        let poll_spec = self.create_poll_spec(&other);
        let cmd = SocketCmdSignal::CreateProbe(poll_spec);

        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SocketNotify::ProbeCreated(rx))   => Ok(rx),
            Ok(SocketNotify::ProbeNotCreated(e)) => Err(e),
            Ok(_)                                => Err(other_io_error("unexpected evt")),
            Err(_)                               => Err(other_io_error("evt channel closed"))
        }
    }

    fn create_poll_spec(&self, other: &SocketFacade) -> Vec<PollRequest> {
        vec!(self.create_recv_poll_request(), other.create_recv_poll_request())
    }

    fn create_recv_poll_request(&self) -> PollRequest {
        PollRequest::new_for_recv(self.id)
    }

    fn on_two_way_device_step(&mut self, other: &mut SocketFacade, votes: Vec<(bool, bool)>) -> io::Result<()> {
        let (self_can_recv,_) = votes[0];
        let (other_can_recv,_) = votes[1];

        if self_can_recv && other_can_recv {
            let msg1 = try!(self.recv_msg());
            let msg2 = try!(other.recv_msg());

            try!(other.send_msg(msg1));
            try!(self.send_msg(msg2));
        } else if self_can_recv {
            try!(self.forward_msg(other));
        } else if other_can_recv {
            try!(other.forward_msg(self));
        }

        Ok(())
    }

    fn forward_msg(&mut self, other: &mut SocketFacade) -> io::Result<()> {
        self.recv_msg().and_then(|msg| other.send_msg(msg))
    }
}

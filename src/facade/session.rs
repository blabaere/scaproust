// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;
use std::thread;
use std::sync::mpsc;
use std::time;

use mio;

use global::*;
use event_loop_msg::*;
use facade::socket::*;
use facade::device::*;
use EventLoop;
use core;

/// This is the entry point of scaproust API.
pub struct Session {
    cmd_sender: mio::Sender<EventLoopSignal>,
    evt_receiver: mpsc::Receiver<SessionNotify>
}

impl Session {
    pub fn new() -> io::Result<Session> {
        let mut config = mio::EventLoopConfig::new();

        config.
            notify_capacity(4_096).
            messages_per_tick(256).
            timer_tick_ms(15).
            timer_wheel_size(1_024).
            timer_capacity(4_096);
        let mut event_loop = try!(mio::EventLoop::configured(config));
        /*let mut builder = mio::EventLoopBuilder::new();

        builder.
            notify_capacity(4_096).
            messages_per_tick(256).
            timer_tick(time::Duration::from_millis(15)).
            timer_wheel_size(1_024).
            timer_capacity(4_096);

        let mut event_loop = try!(builder.build());*/
        let (tx, rx) = mpsc::channel();
        let session = Session { 
            cmd_sender: event_loop.channel(),
            evt_receiver: rx };

        thread::spawn(move || Session::run_event_loop(&mut event_loop, tx));

        Ok(session)
    }

    fn run_event_loop(event_loop: &mut EventLoop, evt_tx: mpsc::Sender<SessionNotify>) {
        let mut handler = core::session::Session::new(evt_tx);
        let exec = event_loop.run(&mut handler);

        match exec {
            Ok(_) => debug!("event loop exited"),
            Err(e) => error!("event loop failed to run: {}", e)
        }
    }

    fn send_cmd(&self, cmd: SessionCmdSignal) -> Result<(), io::Error> {
        let cmd_sig = CmdSignal::Session(cmd);
        let loop_sig = EventLoopSignal::Cmd(cmd_sig);

        self.cmd_sender.send(loop_sig).map_err(convert_notify_err)
    }

    /// Creates a socket of the specified type, which in turn determines its exact semantics.
    /// See [SocketType](enum.SocketType.html) to get the list of available socket types.
    /// The newly created socket is initially not associated with any endpoints.
    /// In order to establish a message flow at least one endpoint has to be added to the socket 
    /// using [connect](struct.Socket.html#method.connect) and [bind](struct.Socket.html#method.bind) methods.
    pub fn create_socket(&self, socket_type: SocketType) -> io::Result<Socket> {
        let cmd = SessionCmdSignal::CreateSocket(socket_type);

        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SessionNotify::SocketCreated(id, rx)) => Ok(self.new_socket(id, socket_type, rx)),
            Ok(_)                                    => Err(other_io_error("unexpected evt")),
            Err(_)                                   => Err(other_io_error("evt channel closed"))
        }
    }

    fn new_socket(&self, id: SocketId, socket_type: SocketType, rx: mpsc::Receiver<SocketNotify>) -> Socket {
        Socket::new(id, socket_type, self.cmd_sender.clone(), rx)
    }

    pub fn create_relay_device(&self, socket: Socket) -> io::Result<Box<Device>> {
        Ok(box RelayDevice::new(socket))
    }

    pub fn create_bridge_device(&self, left: Socket, right: Socket) -> io::Result<Box<Device>> {
        if !left.matches(&right) {
            return Err(other_io_error("Socket types do not match"));
        }

        match left.get_socket_type() {
            SocketType::Pull | SocketType::Sub => self.create_one_way_device(left, right),
            SocketType::Push | SocketType::Pub => self.create_one_way_device(right, left),
            SocketType::Req        |
            SocketType::Rep        |
            SocketType::Respondent |
            SocketType::Surveyor   |
            SocketType::Bus        |
            SocketType::Pair       => self.create_two_way_device(left, right)
        }
    }

    fn create_one_way_device(&self, left: Socket, right: Socket) -> io::Result<Box<Device>> {
        Ok(box OneWayDevice::new(left, right))
    }

    fn create_two_way_device(&self, mut left: Socket, mut right: Socket) -> io::Result<Box<Device>> {
        try!(left.set_option(SocketOption::DeviceItem(true)));
        try!(right.set_option(SocketOption::DeviceItem(true)));
        let cmd = SessionCmdSignal::CreateProbe(left.get_id(), right.get_id());

        try!(self.send_cmd(cmd));

        match self.evt_receiver.recv() {
            Ok(SessionNotify::ProbeCreated(id, rx)) => Ok(self.new_device(id, rx, left, right)),
            Ok(SessionNotify::ProbeNotCreated(e))   => Err(e),
            Ok(_)                                   => Err(other_io_error("unexpected evt")),
            Err(_)                                  => Err(other_io_error("evt channel closed"))
        }
    }

    fn new_device(&self, id: ProbeId, rx: mpsc::Receiver<ProbeNotify>, left: Socket, right: Socket) -> Box<Device> {
        box TwoWayDevice::new(id, self.cmd_sender.clone(), rx, left, right)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.send_cmd(SessionCmdSignal::Shutdown);
    }
}

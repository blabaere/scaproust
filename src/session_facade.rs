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
use socket_facade::*;
use device_facade::*;
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

    pub fn create_relay_device(&self, socket: SocketFacade) -> io::Result<Box<DeviceFacade>> {
        Ok(box RelayDevice::new(socket))
    }

    pub fn create_bridge_device(&self, left: SocketFacade, right: SocketFacade) -> io::Result<Box<DeviceFacade>> {
        if !left.matches(&right) {
            other_io_error("Socket types do not match");
        }

        match left.get_socket_type() {
            SocketType::Pull | SocketType::Sub => Ok(create_one_way_device(left, right)),
            SocketType::Push | SocketType::Pub => Ok(create_one_way_device(right, left)),
            SocketType::Req        |
            SocketType::Rep        |
            SocketType::Respondent |
            SocketType::Surveyor   |
            SocketType::Bus        |
            SocketType::Pair       => unimplemented!()
        }
    }
}

fn create_one_way_device(left: SocketFacade, right: SocketFacade) -> Box<DeviceFacade> {
    box OneWayDevice::new(left, right)
}

/*fn create_two_way_device(left: SocketFacade, right: SocketFacade) -> Box<DeviceFacade> {
    box OneWayDevice::new(left, right)
}*/

impl Drop for SessionFacade {
    fn drop(&mut self) {
        let _ = self.send_cmd(SessionCmdSignal::Shutdown);
    }
}

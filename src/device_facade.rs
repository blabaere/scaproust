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
use socket_facade::*;


pub trait DeviceFacade : Send {
    fn run(mut self: Box<Self>) -> io::Result<()>;
}

///////////////////////////////////////////////////////////////////////////////
//                                                                           //
// RELAY DEVICE                                                              //
//                                                                           //
///////////////////////////////////////////////////////////////////////////////

pub struct RelayDevice {
    socket: Option<SocketFacade>
}

impl RelayDevice {
    pub fn new(s: SocketFacade) -> RelayDevice {
        RelayDevice {
            socket: Some(s)
        }
    }
}

impl DeviceFacade for RelayDevice {
    fn run(mut self: Box<Self>) -> io::Result<()> {
        let mut socket = self.socket.take().unwrap();
        loop {
            try!(socket.recv_msg().and_then(|msg| socket.send_msg(msg)));
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
//                                                                           //
// ONE-WAY BRIDGE DEVICE                                                     //
//                                                                           //
///////////////////////////////////////////////////////////////////////////////

pub struct OneWayDevice {
    left: Option<SocketFacade>,
    right: Option<SocketFacade>
}

impl OneWayDevice {
    pub fn new(l: SocketFacade, r: SocketFacade) -> OneWayDevice {
        OneWayDevice {
            left: Some(l),
            right: Some(r)
        }
    }
}

impl DeviceFacade for OneWayDevice {
    fn run(mut self: Box<Self>) -> io::Result<()> {
        let mut left = self.left.take().unwrap();
        let mut right = self.right.take().unwrap();

        loop {
            try!(left.forward_msg(&mut right))
        }
    }
}

///////////////////////////////////////////////////////////////////////////////
//                                                                           //
// TWO-WAY BRIDGE DEVICE                                                     //
//                                                                           //
///////////////////////////////////////////////////////////////////////////////

pub struct TwoWayDevice {
    id: ProbeId,
    cmd_sender: Sender<EventLoopSignal>,
    evt_receiver: Receiver<PollResult>
}

impl TwoWayDevice {
    pub fn new(id: ProbeId, cmd_tx: Sender<EventLoopSignal>, evt_tx: Receiver<PollResult>) -> TwoWayDevice {
        TwoWayDevice {
            id: id,
            cmd_sender: cmd_tx,
            evt_receiver: evt_tx
        }
    }
}

impl DeviceFacade for TwoWayDevice {
    fn run(mut self: Box<Self>) -> io::Result<()> {
        unimplemented!();
    }
}

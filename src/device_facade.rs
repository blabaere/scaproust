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


pub trait DeviceFacade {
    fn run(mut self: Box<Self>) -> io::Result<()>;
}

pub struct RelayDevice {
    socket: SocketFacade
}

impl RelayDevice {
    pub fn new(s: SocketFacade) -> RelayDevice {
        RelayDevice {
            socket: s
        }
    }
}

impl DeviceFacade for RelayDevice {
    fn run(mut self: Box<Self>) -> io::Result<()> {
        self.socket.run_relay_device()
    }
}

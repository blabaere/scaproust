// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;

use super::socket::Socket;

pub trait Device : Send {
    fn run(self: Box<Self>) -> io::Result<()>;
}

/*****************************************************************************/
/*                                                                           */
/* RELAY DEVICE                                                              */
/*                                                                           */
/*****************************************************************************/

pub struct Relay {
    socket: Option<Socket>
}

impl Relay {
    pub fn new(s: Socket) -> Relay {
        Relay { socket: Some(s) }
    }
}

impl Device for Relay {
    fn run(mut self: Box<Self>) -> io::Result<()> {
        let mut socket = self.socket.take().unwrap();
        loop {
            try!(socket.recv_msg().and_then(|msg| socket.send_msg(msg)));
        }
    }
}

// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io::Result;

use core::{SocketId, EndpointId, Message};

pub trait Network {
    fn connect(&mut self, sid: SocketId, url: &str, pids: (u16, u16)) -> Result<EndpointId>;
    fn reconnect(&mut self, sid: SocketId, eid: EndpointId, url: &str, pids: (u16, u16)) -> Result<()>;
    fn bind(&mut self, sid: SocketId, url: &str, pids: (u16, u16)) -> Result<EndpointId>;
    fn rebind(&mut self, sid: SocketId, eid: EndpointId, url: &str, pids: (u16, u16)) -> Result<()>;
    fn open(&mut self, eid: EndpointId, remote: bool);
    fn close(&mut self, eid: EndpointId, remote: bool);
    fn send(&mut self, eid: EndpointId, msg: Rc<Message>);
    fn recv(&mut self, eid: EndpointId);
}
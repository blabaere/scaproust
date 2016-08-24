// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use super::socket::SocketId;
use super::endpoint::EndpointId;
use message::Message;

pub trait Network {
    fn connect(&mut self, socket_id: SocketId, url: &str, pids: (u16, u16)) -> io::Result<EndpointId>;
    fn bind(&mut self, socket_id: SocketId, url: &str, pids: (u16, u16)) -> io::Result<EndpointId>;
    fn open(&mut self, endpoint_id: EndpointId);
    fn close(&mut self, endpoint_id: EndpointId);
    fn send(&mut self, endpoint_id: EndpointId, msg: Rc<Message>);
    fn recv(&mut self, endpoint_id: EndpointId);
}
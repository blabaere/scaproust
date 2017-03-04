// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io::Result;

use super::{EndpointTmpl};
use core::{SocketId, EndpointId, Message};

pub trait Network {
    fn connect(&mut self, sid: SocketId, tmpl: &EndpointTmpl) -> Result<EndpointId>;
    fn reconnect(&mut self, sid: SocketId, eid: EndpointId, tmpl: &EndpointTmpl) -> Result<()>;
    fn bind(&mut self, sid: SocketId, tmpl: &EndpointTmpl) -> Result<EndpointId>;
    fn rebind(&mut self, sid: SocketId, eid: EndpointId, tmpl: &EndpointTmpl) -> Result<()>;
    fn open(&mut self, eid: EndpointId, remote: bool);
    fn close(&mut self, eid: EndpointId, remote: bool);
    fn send(&mut self, eid: EndpointId, msg: Rc<Message>);
    fn recv(&mut self, eid: EndpointId);
}
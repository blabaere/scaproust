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
    // Network can NOT return instances of transport Endpoint trait
    // because its methods require a reference to a registrar,
    // which does not belong into the core, being optional, io related and mio specific. 
    fn connect(&mut self, socket_id: SocketId, url: &str, pids: (u16, u16)) -> io::Result<EndpointId>;
    fn bind(&mut self, socket_id: SocketId, url: &str, pids: (u16, u16)) -> io::Result<EndpointId>;

    // To implement endpoint operations, one must me able able to pass a transport::Context<PipeEvt>
    // which is a transport::Registrar, meaning a reference to the EventLoop or the Poll is also need
    // Since one can only get a transient reference to them, 
    // it means this type should be allocated on each call. 
    // Implementor do require a mutable access to the transport endpoints collection.
    fn open(&mut self, endpoint_id: EndpointId);
    fn close(&mut self, endpoint_id: EndpointId);
    fn send(&mut self, endpoint_id: EndpointId, msg: Rc<Message>);
    fn recv(&mut self, endpoint_id: EndpointId);
}
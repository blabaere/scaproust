// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::boxed::FnBox;
use std::sync::mpsc::Sender;

use core::socket::Reply;
use core::endpoint::{ Pipe, EndpointId };
use core::network::Network;
use core::message::Message;

pub type ProtocolCtor = Box<FnBox(Sender<Reply>) -> Box<Protocol> + Send>;

pub trait Protocol {
    fn id(&self) -> u16;
    fn peer_id(&self) -> u16;

    fn add_pipe(&mut self, network: &mut Network, eid: EndpointId, pipe: Pipe);
    fn remove_pipe(&mut self, network: &mut Network, eid: EndpointId) -> Option<Pipe>;

    fn send(&mut self, network: &mut Network, msg: Message);
    fn on_send_ack(&mut self, network: &mut Network, eid: EndpointId);
    
    fn recv(&mut self, network: &mut Network);
    fn on_recv_ack(&mut self, network: &mut Network, eid: EndpointId, msg: Message);
}

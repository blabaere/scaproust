// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::cell::RefCell;
use std::io::Result;
use std::time::Duration;

use super::{SocketId, EndpointId, Message, EndpointTmpl, EndpointSpec, EndpointDesc, };
use super::endpoint::{Pipe, Acceptor};
use super::context::{Context, Scheduler, Schedulable, Scheduled, Event};
use super::network::Network;
use io_error::*;

pub fn new_test_pipe(id: EndpointId) -> Pipe {
    Pipe::new_accepted(id, new_test_endpoint_desc())
}

pub fn new_test_endpoint_desc() -> EndpointDesc {
    EndpointDesc {
        send_priority: 0,
        recv_priority: 0,
        tcp_no_delay: false,
        recv_max_size: 1024
    }
}

#[derive(Debug)]
pub struct TestContextSensor {
    close_calls: Vec<(EndpointId, bool)>
}

impl Default for TestContextSensor {
    fn default() -> TestContextSensor {
        TestContextSensor {
            close_calls: Vec::new()
        }
    }
}

impl TestContextSensor {
    fn push_close_call(&mut self, eid: EndpointId, remote: bool) {
        self.close_calls.push((eid, remote))
    }

    pub fn get_close_calls(&self) -> &[(EndpointId, bool)] {
        &self.close_calls
    }
}

#[derive(Debug)]
pub struct TestContext {
    sensor: Rc<RefCell<TestContextSensor>>,
}

impl TestContext {
    pub fn with_sensor(sensor: Rc<RefCell<TestContextSensor>>) -> TestContext {
        TestContext {
            sensor: sensor
        }
    }
}

impl Network for TestContext {
    fn connect(&mut self, sid: SocketId, tmpl: &EndpointTmpl) -> Result<EndpointId> {
        unimplemented!();
    }
    fn reconnect(&mut self, sid: SocketId, eid: EndpointId, tmpl: &EndpointTmpl) -> Result<()> {
        unimplemented!();
    }
    fn bind(&mut self, sid: SocketId, tmpl: &EndpointTmpl) -> Result<EndpointId> {
        unimplemented!();
    }
    fn rebind(&mut self, sid: SocketId, eid: EndpointId, tmpl: &EndpointTmpl) -> Result<()> {
        unimplemented!();
    }
    fn open(&mut self, eid: EndpointId, remote: bool) {
        unimplemented!();
    }
    fn close(&mut self, eid: EndpointId, remote: bool) {
        self.sensor.borrow_mut().push_close_call(eid, remote)
    }
    fn send(&mut self, eid: EndpointId, msg: Rc<Message>) {
        unimplemented!();
    }
    fn recv(&mut self, eid: EndpointId) {
        unimplemented!();
    }
}

impl Scheduler for TestContext {
    fn schedule(&mut self, schedulable: Schedulable, delay: Duration) -> Result<Scheduled> {
        unimplemented!();
    }
    fn cancel(&mut self, scheduled: Scheduled) {
        unimplemented!();
    }
}
impl Context for TestContext {
    fn raise(&mut self, evt: Event) {
        unimplemented!();
    }
}

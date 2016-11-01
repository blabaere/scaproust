// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::fmt;
use std::rc::Rc;
use std::cell::RefCell;
use std::io::Result;
use std::time::Duration;

use super::{SocketId, EndpointId, Message, EndpointTmpl, EndpointDesc, };
use super::endpoint::Pipe;
use super::context::{Context, Scheduler, Schedulable, Scheduled, Event};
use super::network::Network;
use io_error;

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
    close_calls: Vec<(EndpointId, bool)>,
    send_calls: Vec<(EndpointId, Rc<Message>)>,
    recv_calls: Vec<EndpointId>,
    raised_events: Vec<Event>,
    schedule_cancellations: Vec<Scheduled>
}

impl Default for TestContextSensor {
    fn default() -> TestContextSensor {
        TestContextSensor {
            close_calls: Vec::new(),
            send_calls: Vec::new(),
            recv_calls: Vec::new(),
            raised_events: Vec::new(),
            schedule_cancellations: Vec::new()
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

    fn push_send_call(&mut self, eid: EndpointId, msg: Rc<Message>) {
        self.send_calls.push((eid, msg))
    }

    pub fn get_send_calls(&self) -> &[(EndpointId, Rc<Message>)] {
        &self.send_calls
    }

    pub fn assert_no_send_call(&self) {
        assert_eq!(0, self.send_calls.len());
    }

    pub fn assert_send_to(&self, eid: EndpointId, times: usize) {
        let count = self.send_calls.iter().filter(|call| call.0 == eid).count();
        assert_eq!(times, count);
    }

    pub fn assert_one_send_to(&self, eid: EndpointId) {
        assert_eq!(1, self.send_calls.len());

        let &(ref id, _) = &self.send_calls[0];
        assert_eq!(eid, *id);
    }

    fn push_raised_event(&mut self, evt: Event) {
        self.raised_events.push(evt)
    }

    pub fn get_raised_events(&self) -> &[Event] {
        &self.raised_events
    }

    pub fn assert_no_event_raised(&self) {
        assert_eq!(0, self.raised_events.len());
    }

    fn push_schedule_cancellation(&mut self, scheduled: Scheduled) {
        self.schedule_cancellations.push(scheduled)
    }

    pub fn assert_one_cancellation(&self, scheduled: Scheduled) {
        assert_eq!(1, self.schedule_cancellations.len());

        let s = &self.schedule_cancellations[0];
        assert_eq!(scheduled, *s);
    }

    fn push_recv_call(&mut self, eid: EndpointId) {
        self.recv_calls.push(eid)
    }

    pub fn assert_no_recv_call(&self) {
        assert_eq!(0, self.recv_calls.len());
    }

    pub fn assert_one_recv_from(&self, eid: EndpointId) {
        assert_eq!(1, self.recv_calls.len());

        let id = &self.recv_calls[0];
        assert_eq!(eid, *id);
    }
}

pub struct TestContext {
    sensor: Rc<RefCell<TestContextSensor>>,
    schedule_result: Option<Scheduled>
}

impl fmt::Debug for TestContext {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TestContext")
    }
}

impl TestContext {
    pub fn with_sensor(sensor: Rc<RefCell<TestContextSensor>>) -> TestContext {
        TestContext {
            sensor: sensor,
            schedule_result: None
        }
    }
}

impl Network for TestContext {
    fn connect(&mut self, _: SocketId, _: &EndpointTmpl) -> Result<EndpointId> {
        unimplemented!();
    }
    fn reconnect(&mut self, _: SocketId, _: EndpointId, _: &EndpointTmpl) -> Result<()> {
        unimplemented!();
    }
    fn bind(&mut self, _: SocketId, _: &EndpointTmpl) -> Result<EndpointId> {
        unimplemented!();
    }
    fn rebind(&mut self, _: SocketId, _: EndpointId, _: &EndpointTmpl) -> Result<()> {
        unimplemented!();
    }
    fn open(&mut self, _: EndpointId, _: bool) {
        unimplemented!();
    }
    fn close(&mut self, eid: EndpointId, remote: bool) {
        self.sensor.borrow_mut().push_close_call(eid, remote)
    }
    fn send(&mut self, eid: EndpointId, msg: Rc<Message>) {
        self.sensor.borrow_mut().push_send_call(eid, msg)
    }
    fn recv(&mut self, eid: EndpointId) {
        self.sensor.borrow_mut().push_recv_call(eid)
    }
}

impl Scheduler for TestContext {
    fn schedule(&mut self, schedulable: Schedulable, delay: Duration) -> Result<Scheduled> {
        if let Some(scheduled) = self.schedule_result.take() {
            Ok(scheduled)
        } else {
            Err(io_error::other_io_error("test"))
        }
    }
    fn cancel(&mut self, scheduled: Scheduled) {
        self.sensor.borrow_mut().push_schedule_cancellation(scheduled)
    }
}
impl Context for TestContext {
    fn raise(&mut self, evt: Event) {
        self.sensor.borrow_mut().push_raised_event(evt)
    }
}

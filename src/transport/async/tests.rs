// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::ops::Deref;
use std::rc::Rc;
use std::cell::RefCell;
use std::io;

use mio;

use core::Message;
use transport::async::*;
use io_error::*;

pub struct TestStepStreamSensor {
    sent_handshakes: Vec<(u16, u16)>,
    received_handshakes: usize,
    start_send_result: Option<bool>,
    resume_send_result: Option<bool>,
    start_recv_result: Option<Message>,
    resume_recv_result: Option<Message>
}

impl TestStepStreamSensor {
    pub fn new() -> TestStepStreamSensor {
        TestStepStreamSensor {
            sent_handshakes: Vec::new(),
            received_handshakes: 0,
            start_send_result: Some(true),
            resume_send_result: None,
            start_recv_result: None,
            resume_recv_result: None
        }
    }

    pub fn get_sent_handshakes(&self) -> &[(u16, u16)] {
        &self.sent_handshakes
    }

    fn push_sent_handshake(&mut self, sent_handshake: (u16, u16)) {
        self.sent_handshakes.push(sent_handshake);
    }

    pub fn get_received_handshakes(&self) -> usize {
        self.received_handshakes
    }

    fn push_received_handshake(&mut self) {
        self.received_handshakes += 1;
    }

    fn take_start_send_result(&mut self) -> Option<bool> {
        self.start_send_result.take()
    }

    pub fn set_start_send_result(&mut self, res: Option<bool>) {
        self.start_send_result = res;
    }

    fn take_resume_send_result(&mut self) -> Option<bool> {
        self.resume_send_result.take()
    }

    pub fn set_resume_send_result(&mut self, res: Option<bool>) {
        self.resume_send_result = res;
    }

    fn take_start_recv_result(&mut self) -> Option<Message> {
        self.start_recv_result.take()
    }

    pub fn set_start_recv_result(&mut self, res: Option<Message>) {
        self.start_recv_result = res;
    }

    fn take_resume_recv_result(&mut self) -> Option<Message> {
        self.resume_recv_result.take()
    }

    pub fn set_resume_recv_result(&mut self, res: Option<Message>) {
        self.resume_recv_result = res;
    }
}

pub struct TestStepStream {
    sensor: Rc<RefCell<TestStepStreamSensor>>,
    send_handshake_ok: bool,
    recv_handshake_ok: bool,
    pending_send: bool,
    pending_recv: bool
}

impl TestStepStream {
    pub fn new() -> TestStepStream {
        let sensor = TestStepStreamSensor::new();
        TestStepStream::with_sensor(Rc::new(RefCell::new(sensor)))
    }
    pub fn with_sensor(sensor: Rc<RefCell<TestStepStreamSensor>>) -> TestStepStream {
        TestStepStream {
            sensor: sensor,
            send_handshake_ok: true,
            recv_handshake_ok: true,
            pending_send: false,
            pending_recv: false
        }
    }
    pub fn set_send_handshake_ok(&mut self, send_handshake_ok: bool) {
        self.send_handshake_ok = send_handshake_ok;
    }
}

impl stub::AsyncPipeStub for TestStepStream {
}

impl mio::Evented for TestStepStream {
    fn register(&self, _: &mio::Poll, _: mio::Token, _: mio::Ready, _: mio::PollOpt) -> io::Result<()> {
        unimplemented!();
    }
    fn reregister(&self, _: &mio::Poll, _: mio::Token, _: mio::Ready, _: mio::PollOpt) -> io::Result<()> {
        unimplemented!();
    }
    fn deregister(&self, _: &mio::Poll) -> io::Result<()> {
        unimplemented!();
    }
}

impl Deref for TestStepStream {
    type Target = mio::Evented;
    fn deref(&self) -> &Self::Target {
        self
    }
}

impl stub::Handshake for TestStepStream {
    fn send_handshake(&mut self, pids: (u16, u16)) -> io::Result<()> {
        self.sensor.borrow_mut().push_sent_handshake(pids);
        if self.send_handshake_ok { Ok(()) } else { Err(other_io_error("test")) }
    }
    fn recv_handshake(&mut self, _: (u16, u16)) -> io::Result<()> {
        self.sensor.borrow_mut().push_received_handshake();
        if self.recv_handshake_ok { Ok(()) } else { Err(other_io_error("test")) }
    }
}

impl stub::Sender for TestStepStream {
    fn start_send(&mut self, _: Rc<Message>) -> io::Result<bool> {
        match self.sensor.borrow_mut().take_start_send_result() {
            Some(true) => { self.pending_send = false; Ok(true) },
            Some(false) => { self.pending_send = true; Ok(false) },
            None => Err(other_io_error("test"))
        }
    }

    fn resume_send(&mut self) -> io::Result<bool> {
        match self.sensor.borrow_mut().take_resume_send_result() {
            Some(true) => { self.pending_send = false; Ok(true) },
            Some(false) => { self.pending_send = true; Ok(false) },
            None => Err(other_io_error("test"))
        }
    }

    fn has_pending_send(&self) -> bool {
        self.pending_send
    }
}

impl stub::Receiver for TestStepStream {
    fn start_recv(&mut self) -> io::Result<Option<Message>> {
        match self.sensor.borrow_mut().take_start_recv_result() {
            Some(msg) => { self.pending_recv = false; Ok(Some(msg)) },
            None =>      { self.pending_recv = true; Ok(None) }
        }
    }

    fn resume_recv(&mut self) -> io::Result<Option<Message>> {
        match self.sensor.borrow_mut().take_resume_recv_result() {
            Some(msg) => { self.pending_recv = false; Ok(Some(msg)) },
            None =>      { self.pending_recv = true; Ok(None) }
        }
    }

    fn has_pending_recv(&self) -> bool {
        self.pending_recv
    }
}

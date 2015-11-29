// Copyright 015 Copyright (c) 015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use byteorder::{ BigEndian, WriteBytesExt };

use Message;
use transport::Connection;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum SendOperationStep {
    Prefix,
    Header,
    Body,
    Done
}

impl SendOperationStep {
    fn next(&self) -> SendOperationStep {
        match *self {
            SendOperationStep::Prefix => SendOperationStep::Header,
            SendOperationStep::Header => SendOperationStep::Body,
            SendOperationStep::Body   => SendOperationStep::Done,
            SendOperationStep::Done   => SendOperationStep::Done
        }
    }
}

pub struct SendOperation {
    prefix: Vec<u8>,
    msg: Rc<Message>,
    step: SendOperationStep,
    written: usize
}

impl SendOperation {
    pub fn new(msg: Rc<Message>) -> io::Result<SendOperation> {
        let mut prefix = Vec::with_capacity(8);
        let msg_len = msg.len() as u64;

        try!(prefix.write_u64::<BigEndian>(msg_len));

        Ok(SendOperation {
            prefix: prefix,
            msg: msg,
            step: SendOperationStep::Prefix,
            written: 0
        })
    }

    fn step_forward(&mut self) {
        self.step = self.step.next();
        self.written = 0;
    }

    pub fn send(&mut self, connection: &mut Connection) -> io::Result<bool> {
        // try send size prefix
        if self.step == SendOperationStep::Prefix {
            if try!(self.send_buffer_and_check(connection)) {
                self.step_forward();
            } else {
                return Ok(false);
            }
        }

        // try send msg header
        if self.step == SendOperationStep::Header {
            if try!(self.send_buffer_and_check(connection)) {
                self.step_forward();
            } else {
                return Ok(false);
            }
        }

        // try send msg body
        if self.step == SendOperationStep::Body {
            if try!(self.send_buffer_and_check(connection)) {
                self.step_forward();
            } else {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn send_buffer_and_check(&mut self, connection: &mut Connection) -> io::Result<bool> {
        let buffer: &[u8] = match self.step {
            SendOperationStep::Prefix => &self.prefix,
            SendOperationStep::Header => self.msg.get_header(),
            SendOperationStep::Body => self.msg.get_body(),
            _ => return Ok(true)
        };

        self.written += try!(self.send_buffer(connection, buffer));

        Ok(self.written == buffer.len())
    }

    fn send_buffer(&self, connection: &mut Connection, buffer: &[u8]) -> io::Result<usize> {
        let remaining = buffer.len() - self.written;

        if remaining > 0 {
            let fragment = &buffer[self.written..];
            let written = match try!(connection.try_write(fragment)) {
                Some(x) => x,
                None => 0
            };

            Ok(written)
        } else {
            Ok(0)
        }
    }
}
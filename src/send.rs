// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use byteorder::{ BigEndian, WriteBytesExt };

use Message;
use transport::Connection;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Step {
    Prefix,
    Header,
    Body,
    Done
}

impl Step {
    fn next(&self) -> Step {
        match *self {
            Step::Prefix => Step::Header,
            Step::Header => Step::Body,
            Step::Body   => Step::Done,
            Step::Done   => Step::Done
        }
    }
}

pub struct SendOperation {
    prefix: Vec<u8>,
    msg: Rc<Message>,
    step: Step,
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
            step: Step::Prefix,
            written: 0
        })
    }

    fn step_forward(&mut self) {
        self.step = self.step.next();
        self.written = 0;
    }

    pub fn send(&mut self, connection: &mut Connection) -> io::Result<bool> {
        // try send size prefix
        if self.step == Step::Prefix {
            if try!(self.send_buffer_and_check(connection)) {
                self.step_forward();
            } else {
                return Ok(false);
            }
        }

        // try send msg header
        if self.step == Step::Header {
            if try!(self.send_buffer_and_check(connection)) {
                self.step_forward();
            } else {
                return Ok(false);
            }
        }

        // try send msg body
        if self.step == Step::Body {
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
            Step::Prefix => &self.prefix,
            Step::Header => self.msg.get_header(),
            Step::Body => self.msg.get_body(),
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
            //debug!("Sent {}/{} bytes.", written, fragment.len());

            Ok(written)
        } else {
            Ok(0)
        }
    }
}
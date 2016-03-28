// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;

use byteorder::*;

use Message;
use transport::Connection;

/*****************************************************************************/
/*                                                                           */
/* NOT WINDOWS, SEND PIECE BY PIECE                                          */
/*                                                                           */
/*****************************************************************************/

#[cfg(not(windows))]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Step {
    Prefix,
    Header,
    Body,
    Done
}

#[cfg(not(windows))]
impl Step {
    fn next(&self) -> Step {
        match *self {
            Step::Prefix              => Step::Header,
            Step::Header              => Step::Body,
            Step::Body | Step::Done   => Step::Done
        }
    }
}

#[cfg(not(windows))]
pub struct SendOperation {
    prefix: [u8; 8],
    msg: Rc<Message>,
    step: Step,
    written: usize
}

#[cfg(not(windows))]
impl SendOperation {
    pub fn new(msg: Rc<Message>) -> SendOperation {
        let mut prefix = [0u8; 8];
        let msg_len = msg.len() as u64;

        BigEndian::write_u64(&mut prefix, msg_len);

        SendOperation {
            prefix: prefix,
            msg: msg,
            step: Step::Prefix,
            written: 0
        }
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

            Ok(written)
        } else {
            Ok(0)
        }
    }
}

/*****************************************************************************/
/*                                                                           */
/* WINDOWS, COPY EVERYTHING IN A BUFFER AND SEND IT                          */
/*                                                                           */
/*****************************************************************************/

#[cfg(windows)]
pub struct SendOperation {
    buffer: Vec<u8>,
    written: usize
}

#[cfg(windows)]
impl SendOperation {
    pub fn new(msg: Rc<Message>) -> SendOperation {
        let msg_len = msg.len();
        let buf_len = 8 + msg_len;
        let mut buffer = Vec::with_capacity(buf_len);
        let mut prefix = [0u8; 8];

        BigEndian::write_u64(&mut prefix, msg_len as u64);
        buffer.extend_from_slice(&prefix);
        buffer.extend_from_slice(msg.get_header());
        buffer.extend_from_slice(msg.get_body());

        SendOperation {
            buffer: buffer,
            written: 0
        }
    }

    pub fn send(&mut self, connection: &mut Connection) -> io::Result<bool> {
        if self.done() {
            return Ok(true);
        }
        
        let fragment = &self.buffer[self.written..];
        let written = match try!(connection.try_write(fragment)) {
            Some(x) => x,
            None => 0
        };

        self.written += written;

        Ok(self.done())
    }

    fn done(&self) -> bool {
        self.written == self.buffer.len()
    }
}

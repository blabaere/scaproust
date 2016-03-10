// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;

use byteorder::*;

use Message;
use transport::Connection;
use global;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Step {
    Prefix,
    Payload,
    Done
}

impl Step {
    fn next(&self) -> Step {
        match *self {
            Step::Prefix               => Step::Payload,
            Step::Payload | Step::Done => Step::Done
        }
    }
}

pub struct RecvOperation {
    step: Step,
    read: usize,
    prefix: [u8; 8],
    msg_len: u64,
    buffer: Option<Vec<u8>>
}

impl RecvOperation {

    pub fn new() -> RecvOperation {
        RecvOperation {
            step: Step::Prefix,
            read: 0,
            prefix: [0u8; 8],
            msg_len: 0,
            buffer: None
        }
    }

    fn step_forward(&mut self) {
        self.step = self.step.next();
        self.read = 0;
    }

    pub fn recv(&mut self, connection: &mut Connection) -> io::Result<Option<Message>> {
        if self.step == Step::Prefix {
            self.read += try!(RecvOperation::recv_buffer(connection, &mut self.prefix[self.read..]));

            if self.read == self.prefix.len() {
                self.step_forward();
                self.msg_len = BigEndian::read_u64(&self.prefix);
                self.buffer = Some(vec![0u8; self.msg_len as usize]);
            } else {
                return Ok(None);
            }
        }

        if self.step == Step::Payload {
            let mut buffer = self.buffer.take().unwrap();

            self.read += try!(RecvOperation::recv_buffer(connection, &mut buffer[self.read..]));

            if self.read as u64 == self.msg_len {
                self.step_forward();

                return Ok(Some(Message::with_body(buffer)));
            } else {
                self.buffer = Some(buffer);

                return Ok(None);
            }
        }

        Err(global::other_io_error("recv operation already completed"))
    }

    fn recv_buffer(connection: &mut Connection, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.len() > 0 {
            let read = match try!(connection.try_read(buffer)) {
                Some(x) => x,
                None => 0
            };

            Ok(read)
        } else {
            Ok(0)
        }
    }
}

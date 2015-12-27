// Copyright 2015 Copyright (c) 2015 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;

use byteorder::{ BigEndian, ReadBytesExt };

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
            Step::Prefix  => Step::Payload,
            Step::Payload => Step::Done,
            Step::Done    => Step::Done
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

            if self.read == 0 {
                return Ok(None);
            } else if self.read == self.prefix.len() {
                self.step_forward();
                let mut bytes: &[u8] = &mut self.prefix;
                self.msg_len = try!(bytes.read_u64::<BigEndian>());
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

            debug!("Received {}/{} bytes.", read, buffer.len());

            Ok(read)
        } else {
            debug!("Did not read anything because buffer is empty !");
            Ok(0)
        }
    }
}

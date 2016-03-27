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

pub struct SendOperation {
    buffer: Vec<u8>,
    written: usize
}

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

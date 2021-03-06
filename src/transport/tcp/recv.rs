// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;

use byteorder::{ BigEndian, ByteOrder };

use core::Message;
use transport::async::stub::*;
use io_error::*;

pub struct RecvOperation {
    step: Option<RecvOperationStep>
}

impl RecvOperation {
    pub fn new(recv_max_size: u64) -> RecvOperation {
        RecvOperation {
            step: Some(RecvOperationStep::Header([0; 8], 0, recv_max_size))
        }
    }

    pub fn run<T:io::Read>(&mut self, stream: &mut T) -> io::Result<Option<Message>> {
        if let Some(step) = self.step.take() {
            self.resume_at(stream, step)
        } else {
            Err(other_io_error("Cannot resume already finished recv operation"))
        }
    }

    fn resume_at<T:io::Read>(&mut self, stream: &mut T, step: RecvOperationStep) -> io::Result<Option<Message>> {
        let mut cur_step = step;

        loop {
            let (passed, next_step) = cur_step.advance(stream)?;

            if !passed {
                self.step = Some(next_step);
                return Ok(None);
            }

            match next_step {
                RecvOperationStep::Terminal(msg) => return Ok(Some(msg)),
                other => cur_step = other
            }
        }
    }
}

enum RecvOperationStep {
    Header([u8; 8], usize, u64),
    Payload(Vec<u8>, usize),
    Terminal(Message)
}

impl RecvOperationStep {
    fn advance<T:io::Read>(self, stream: &mut T) -> io::Result<(bool, RecvOperationStep)> {
        match self {
            RecvOperationStep::Header(buffer, read, max_size) => read_header(stream, buffer, read, max_size),
            RecvOperationStep::Payload(buffer, read) => read_payload(stream, buffer, read),
            RecvOperationStep::Terminal(_) => Err(other_io_error("Cannot advance terminal step of recv operation"))
        }
    }
}

fn read_header<T:io::Read>(stream: &mut T, mut buffer: [u8; 8], mut read: usize, max_size: u64) -> io::Result<(bool, RecvOperationStep)> {
    read += stream.read_buffer(&mut buffer[read..])?;

    if read == 8 {
        let msg_len = BigEndian::read_u64(&buffer);
        if max_size > 0 && msg_len > max_size {
            Err(invalid_data_io_error("message is too long"))
        } else {
            let payload = vec![0u8; msg_len as usize];

            Ok((true, RecvOperationStep::Payload(payload, 0)))
        }
    } else {
        Ok((false, RecvOperationStep::Header(buffer, read, max_size)))
    }
}

fn read_payload<T:io::Read>(stream: &mut T, mut buffer: Vec<u8>, mut read: usize) -> io::Result<(bool, RecvOperationStep)> {
    read += stream.read_buffer(&mut buffer[read..])?;

    if read == buffer.capacity() {
        Ok((true, RecvOperationStep::Terminal(Message::from_body(buffer))))
    } else {
        Ok((false, RecvOperationStep::Payload(buffer, read)))
    }
}

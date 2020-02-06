// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::rc::Rc;
use std::io;
use std::io::Write;

use byteorder::{ BigEndian, ByteOrder };

use mio::tcp::TcpStream;
use iovec::IoVec;

use core::Message;
use io_error::*;

pub struct SendOperation {
    step: Option<SendOperationStep>
}

impl SendOperation {
    pub fn new(msg: Rc<Message>) -> SendOperation {
        SendOperation { 
            step: Some(SendOperationStep::TransportHdr(msg, 0))
        }
    }

    pub fn run(&mut self, stream: &mut TcpStream) -> io::Result<bool> {
        if let Some(step) = self.step.take() {
            self.resume_at(stream, step)
        } else {
            Err(other_io_error("Cannot resume already finished send operation"))
        }
    }

    fn resume_at(&mut self, stream: &mut TcpStream, step: SendOperationStep) -> io::Result<bool> {
        let mut cur_step = step;

        loop {
            let (passed, next_step) = cur_step.advance(stream)?;

            if next_step.is_terminal() {
                return Ok(true);
            }
            if !passed {
                self.step = Some(next_step);
                return Ok(false)
            }

            cur_step = next_step;
        }
    }
}

enum SendOperationStep {
    TransportHdr(Rc<Message>, usize),
    ProtocolHdr(Rc<Message>, usize),
    UsrPayload(Rc<Message>, usize),
    Terminal
}

impl SendOperationStep {
    /// Writes the buffers composing the message to the specified stream.
    /// Returns whether the step has passed, and what is the next step.
    fn advance(self, stream: &mut TcpStream) -> io::Result<(bool, SendOperationStep)> {
        match self {
            SendOperationStep::TransportHdr(msg, written) => write_transport_hdr(stream, msg, written),
            SendOperationStep::ProtocolHdr(msg, written) => write_protocol_hdr(stream, msg, written),
            SendOperationStep::UsrPayload(msg, written) => write_usr_payload(stream, msg, written),
            SendOperationStep::Terminal => Err(other_io_error("Cannot advance terminal step of send operation"))
        }
    }

    fn is_terminal(&self) -> bool {
        match *self {
            SendOperationStep::Terminal => true,
            _ => false,
        }
    }
}

fn write_transport_hdr(stream: &mut TcpStream, msg: Rc<Message>, mut written: usize) -> io::Result<(bool, SendOperationStep)> {
    let mut buffer = [0u8; 8];

    BigEndian::write_u64(&mut buffer, msg.len() as u64);

    let transport_hdr = if written == 0 {
        &buffer
    } else {
        &buffer[written..]
    };

    if msg.get_header().len() == 0 {
        let payload = msg.get_body();

        written += if payload.len() == 0 {
            write_buffer(stream, transport_hdr)?
        } else {
            let buffers: &[&IoVec] = &[transport_hdr.into(), payload.into()];

            write_buffers(stream, buffers)?
        };
    } else {
        let proto_hdr = msg.get_header();
        let payload = msg.get_body();

        written += if payload.len() == 0 {
            let buffers: &[&IoVec] = &[transport_hdr.into(), proto_hdr.into()];
            write_buffers(stream, buffers)?
        } else {
            let buffers: &[&IoVec] = &[transport_hdr.into(), proto_hdr.into(), payload.into()];
            write_buffers(stream, buffers)?
        };
    }

    let transport_limit = 8;
    let proto_hdr_limit = transport_limit + msg.get_header().len();
    let payload_limit = proto_hdr_limit + msg.get_body().len();

    if written < transport_limit {
        Ok((false, SendOperationStep::TransportHdr(msg, written)))
    } else if written < proto_hdr_limit {
        Ok((false, SendOperationStep::ProtocolHdr(msg, written - transport_limit)))
    } else if written < payload_limit {
        Ok((false, SendOperationStep::UsrPayload(msg, written - proto_hdr_limit)))
    } else {
        Ok((true, SendOperationStep::Terminal))
    }
}

fn write_protocol_hdr(stream: &mut TcpStream, msg: Rc<Message>, mut written: usize) -> io::Result<(bool, SendOperationStep)> {
    if written == 0 {
        let proto_hdr = msg.get_header();
        let payload = msg.get_body();
        let buffers: &[&IoVec] = &[proto_hdr.into(), payload.into()];

        written += write_buffers(stream, buffers)?;
    } else {
        let proto_hdr = msg.get_header();
        let proto_hdr = &proto_hdr[written..];
        let payload = msg.get_body();
        let buffers: &[&IoVec] = &[proto_hdr.into(), payload.into()];

        written += write_buffers(stream, buffers)?;
    }

    let proto_hdr_limit = msg.get_header().len();
    let payload_limit = proto_hdr_limit + msg.get_body().len();

    if written < proto_hdr_limit {
        Ok((false, SendOperationStep::ProtocolHdr(msg, written)))
    } else if written < payload_limit {
        Ok((false, SendOperationStep::UsrPayload(msg, written - proto_hdr_limit)))
    } else {
        Ok((true, SendOperationStep::Terminal))
    }
}

fn write_usr_payload(stream: &mut TcpStream, msg: Rc<Message>, mut written: usize) -> io::Result<(bool, SendOperationStep)> {
    if written == 0 {
        let payload = msg.get_body();

        written += write_buffer(stream, payload)?;
    } else {
        let payload = msg.get_body();
        let payload = &payload[written..];

        written += write_buffer(stream, payload)?;
    }

    let payload_limit = msg.get_body().len();

    if written < payload_limit {
        Ok((false, SendOperationStep::UsrPayload(msg, written)))
    } else {
        Ok((true, SendOperationStep::Terminal))
    }
}

fn write_buffer(stream: &mut TcpStream, buffer: &[u8]) -> io::Result<usize> {
    flatten_would_block(stream.write(buffer))
}

fn write_buffers(stream: &mut TcpStream, buffers: &[&IoVec]) -> io::Result<usize> {
    flatten_would_block(stream.write_bufs(buffers))
}

fn flatten_would_block(result: io::Result<usize>) -> io::Result<usize> {
    match result {
        Ok(x)  => Ok(x),
        Err(e) => {
            if e.kind() == io::ErrorKind::WouldBlock {
                Ok(0)
            } else {
                Err(e)
            }
        }
    }
}

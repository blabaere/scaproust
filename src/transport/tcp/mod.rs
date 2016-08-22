// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::ops::Deref;
use std::str::FromStr;
use std::rc::Rc;
use std::io;
use std::net;

use super::*;
use super::stream::*;
use util::*;
use Message;

use mio;
use mio::tcp::{TcpListener, TcpStream, Shutdown};

mod send;

/*****************************************************************************/
/*                                                                           */
/* Transport                                                                 */
/*                                                                           */
/*****************************************************************************/

pub struct Tcp;

impl Tcp {
    fn connect(&self, addr: &net::SocketAddr) -> io::Result<Box<Endpoint<PipeCmd, PipeEvt>>> {
        let stream = try!(TcpStream::connect(addr));
        let step_stream = TcpStepStream::new(stream);
        let pipe = box Pipe::new(step_stream, 0, 0);

        Ok(pipe)
    }
}

impl Transport for Tcp {
    fn connect(&self, url: &str) -> io::Result<Box<Endpoint<PipeCmd, PipeEvt>>> {
        match net::SocketAddr::from_str(url) {
            Ok(addr) => self.connect(&addr),
            Err(_) => Err(invalid_input_io_error(url))
        }
    }

    fn bind(&self, url: &str) -> io::Result<Box<Endpoint<AcceptorCmd, AcceptorEvt>>> {
        Err(other_io_error("Not implemented"))
    }
}

/*****************************************************************************/
/*                                                                           */
/* Step Stream                                                               */
/*                                                                           */
/*****************************************************************************/

struct TcpStepStream {
    stream: TcpStream,
    send_operation: Option<send::SendOperation>, // usize => TcpSendOperation ...
    recv_operation: Option<usize>
}

impl Deref for TcpStepStream {
    type Target = mio::Evented;
    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl TcpStepStream {
    fn new(stream: TcpStream) -> TcpStepStream {
        TcpStepStream {
            stream: stream,
            send_operation: None,
            recv_operation: None
        }
    }
}

impl Drop for TcpStepStream {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

impl Sender for TcpStepStream {
    fn start_send(&mut self, msg: Rc<Message>) -> io::Result<bool> {
        Err(other_io_error("Not implemented"))
    }
    fn resume_send(&mut self) -> io::Result<bool> {
        Err(other_io_error("Not implemented"))
    }
    fn has_pending_send(&self) -> bool { false }
}

impl Receiver for TcpStepStream {
    fn start_recv(&mut self) -> io::Result<Option<Message>> {
        Err(other_io_error("Not implemented"))
    }
    fn resume_recv(&mut self) -> io::Result<Option<Message>> {
        Err(other_io_error("Not implemented"))
    }
    fn has_pending_recv(&self) -> bool { false }
}

impl Handshake for TcpStepStream {
    fn send_handshake(&mut self, proto_id: u16) -> io::Result<()>{
        Err(other_io_error("Not implemented"))
    }
    fn recv_handshake(&mut self, proto_id: u16) -> io::Result<()>{
        Err(other_io_error("Not implemented"))
    }
}

impl StepStream for TcpStepStream {
}

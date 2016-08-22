// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

mod send;
mod recv;

use std::ops::Deref;
use std::str::FromStr;
use std::rc::Rc;
use std::io;
use std::net;

use mio;
use mio::tcp::{TcpListener, TcpStream, Shutdown};

use self::send::SendOperation;
use self::recv::RecvOperation;
use super::*;
use super::stream::*;
use io_error::*;
use Message;

/*****************************************************************************/
/*                                                                           */
/* Transport                                                                 */
/*                                                                           */
/*****************************************************************************/

pub struct Tcp;

impl Tcp {
    fn connect(&self, addr: &net::SocketAddr, pids: (u16, u16)) -> io::Result<Box<Endpoint<PipeCmd, PipeEvt>>> {
        let stream = try!(TcpStream::connect(addr));
        let step_stream = TcpStepStream::new(stream);
        let pipe = box Pipe::new(step_stream, pids);

        Ok(pipe)
    }
}

impl Transport for Tcp {
    fn connect(&self, url: &str, pids: (u16, u16)) -> io::Result<Box<Endpoint<PipeCmd, PipeEvt>>> {
        match net::SocketAddr::from_str(url) {
            Ok(addr) => self.connect(&addr, pids),
            Err(_) => Err(invalid_input_io_error(url))
        }
    }

    fn bind(&self, url: &str, pids: (u16, u16)) -> io::Result<Box<Endpoint<AcceptorCmd, AcceptorEvt>>> {
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
    send_operation: Option<SendOperation>,
    recv_operation: Option<RecvOperation>
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

    fn run_send_operation(&mut self, mut send_operation: SendOperation) -> io::Result<bool> {
        if try!(send_operation.run(&mut self.stream)) {
            Ok(true)
        } else {
            self.send_operation = Some(send_operation);
            Ok(false)
        }
    }

    fn run_recv_operation(&mut self, mut recv_operation: RecvOperation) -> io::Result<Option<Message>> {
        match try!(recv_operation.run(&mut self.stream)) {
            Some(msg) => Ok(Some(msg)),
            None => {
                self.recv_operation = Some(recv_operation);
                Ok(None)
            }
        }
    }
}

impl Drop for TcpStepStream {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

/*****************************************************************************/
/*                                                                           */
/* Sender for TcpStepStream                                                  */
/*                                                                           */
/*****************************************************************************/

impl Sender for TcpStepStream {
    fn start_send(&mut self, msg: Rc<Message>) -> io::Result<bool> {
        let send_operation = SendOperation::new(msg);

        self.run_send_operation(send_operation)
    }

    fn resume_send(&mut self) -> io::Result<bool> {
        if let Some(send_operation) = self.send_operation.take() {
            self.run_send_operation(send_operation)
        } else {
            Err(other_io_error("Cannot resume send: no pending operation"))
        }
    }

    fn has_pending_send(&self) -> bool {
        self.send_operation.is_some()
    }
}

/*****************************************************************************/
/*                                                                           */
/* Receiver for TcpStepStream                                                */
/*                                                                           */
/*****************************************************************************/

impl Receiver for TcpStepStream {
    fn start_recv(&mut self) -> io::Result<Option<Message>> {
        let recv_operation = RecvOperation::new(DEFAULT_RECV_MAX_SIZE);

        self.run_recv_operation(recv_operation)
    }

    fn resume_recv(&mut self) -> io::Result<Option<Message>> {
        if let Some(recv_operation) = self.recv_operation.take() {
            self.run_recv_operation(recv_operation)
        } else {
            Err(other_io_error("Cannot resume recv: no pending operation"))
        }
    }

    fn has_pending_recv(&self) -> bool {
        self.recv_operation.is_some()
    }
}

/*****************************************************************************/
/*                                                                           */
/* Handshake for TcpStepStream                                               */
/*                                                                           */
/*****************************************************************************/

impl Handshake for TcpStepStream {
    fn send_handshake(&mut self, pids: (u16, u16)) -> io::Result<()> {
        send_and_check_handshake(&mut self.stream, pids)
    }
    fn recv_handshake(&mut self, pids: (u16, u16)) -> io::Result<()> {
        recv_and_check_handshake(&mut self.stream, pids)
    }
}

impl StepStream for TcpStepStream {
}

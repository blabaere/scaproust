// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::ops::Deref;
use std::rc::Rc;
use std::io;

use mio;
use mio::tcp::{TcpStream, Shutdown};

use core::Message;
use transport::tcp::send::SendOperation;
use transport::tcp::recv::RecvOperation;
use transport::async::stub::*;
use io_error::*;

/*****************************************************************************/
/*                                                                           */
/* TcpPipeStub                                                               */
/*                                                                           */
/*****************************************************************************/

pub struct TcpPipeStub {
    stream: TcpStream,
    recv_max_size: u64,
    send_operation: Option<SendOperation>,
    recv_operation: Option<RecvOperation>
}

impl Deref for TcpPipeStub {
    type Target = mio::Evented;
    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl TcpPipeStub {
    pub fn new(stream: TcpStream, recv_max_size: u64) -> TcpPipeStub {
        TcpPipeStub {
            stream: stream,
            recv_max_size: recv_max_size,
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

impl Drop for TcpPipeStub {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

/*****************************************************************************/
/*                                                                           */
/* Sender for TcpPipeStub                                                    */
/*                                                                           */
/*****************************************************************************/

impl Sender for TcpPipeStub {
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
/* Receiver for TcpPipeStub                                                  */
/*                                                                           */
/*****************************************************************************/

impl Receiver for TcpPipeStub {
    fn start_recv(&mut self) -> io::Result<Option<Message>> {
        let recv_operation = RecvOperation::new(self.recv_max_size);

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
/* Handshake for TcpPipeStub                                                 */
/*                                                                           */
/*****************************************************************************/

impl Handshake for TcpPipeStub {
    fn send_handshake(&mut self, pids: (u16, u16)) -> io::Result<()> {
        send_and_check_handshake(&mut self.stream, pids)
    }
    fn recv_handshake(&mut self, pids: (u16, u16)) -> io::Result<()> {
        recv_and_check_handshake(&mut self.stream, pids)
    }
}

impl AsyncPipeStub for TcpPipeStub {
}

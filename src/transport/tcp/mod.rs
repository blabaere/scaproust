// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

mod step;
mod send;
mod recv;
mod acceptor;

use std::ops::Deref;
use std::str::FromStr;
use std::rc::Rc;
use std::io;
use std::net;

use mio;
use mio::tcp::{TcpListener, TcpStream, Shutdown};

use self::step::TcpStepStream;
use self::send::SendOperation;
use self::recv::RecvOperation;
use self::acceptor::Acceptor;
use transport::*;
use transport::stream::*;
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
    fn bind(&self, addr: &net::SocketAddr, pids: (u16, u16)) -> io::Result<Box<Endpoint<AcceptorCmd, AcceptorEvt>>> {
        let listener = try!(TcpListener::bind(addr));
        let acceptor = box Acceptor::new(listener, pids);

        Ok(acceptor)
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
        match net::SocketAddr::from_str(url) {
            Ok(addr) => self.bind(&addr, pids),
            Err(_) => Err(invalid_input_io_error(url))
        }
    }
}

// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

mod stub;
mod send;
mod recv;
mod acceptor;

use std::str::FromStr;
use std::io;
use std::net;

use mio::tcp::{TcpListener, TcpStream};

use self::stub::TcpPipeStub;
use self::acceptor::TcpAcceptor;
use transport::Transport;
use transport::pipe::Pipe;
use transport::acceptor::Acceptor;
use transport::async::AsyncPipe;
use io_error::*;

pub struct Tcp;

impl Tcp {
    fn connect(&self, addr: &net::SocketAddr, pids: (u16, u16)) -> io::Result<Box<Pipe>> {
        let stream = try!(TcpStream::connect(addr));
        let stub = TcpPipeStub::new(stream);
        let pipe = box AsyncPipe::new(stub, pids);

        Ok(pipe)
    }
    fn bind(&self, addr: &net::SocketAddr, pids: (u16, u16)) -> io::Result<Box<Acceptor>> {
        let listener = try!(TcpListener::bind(addr));
        let acceptor = box TcpAcceptor::new(listener, pids);

        Ok(acceptor)
    }
}

impl Transport for Tcp {
    fn connect(&self, url: &str, pids: (u16, u16)) -> io::Result<Box<Pipe>> {
        match net::SocketAddr::from_str(url) {
            Ok(addr) => self.connect(&addr, pids),
            Err(_) => Err(invalid_input_io_error(url))
        }
    }

    fn bind(&self, url: &str, pids: (u16, u16)) -> io::Result<Box<Acceptor>> {
        match net::SocketAddr::from_str(url) {
            Ok(addr) => self.bind(&addr, pids),
            Err(_) => Err(invalid_input_io_error(url))
        }
    }
}

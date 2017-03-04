// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;

use mio;
use mio::tcp::{TcpListener, TcpStream};

use transport::*;
use transport::acceptor::*;
use transport::async::AsyncPipe;
use super::stub::TcpPipeStub;

pub struct TcpAcceptor {
    listener: TcpListener,
    proto_ids: (u16, u16),
    no_delay: bool,
    recv_max_size: u64
}

impl TcpAcceptor {

    pub fn new(l: TcpListener, dest: &Destination) -> TcpAcceptor {
        TcpAcceptor {
            listener: l,
            proto_ids: dest.pids,
            no_delay: dest.tcp_no_delay,
            recv_max_size: dest.recv_max_size
        }
    }

    fn accept(&mut self, ctx: &mut Context) {
        let mut pipes = Vec::new();

        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    let _ = stream.set_nodelay(self.no_delay);
                    let pipe = self.create_pipe(stream);

                    pipes.push(pipe);
                },
                Err(e) => {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        break;
                    } else {
                        ctx.raise(Event::Error(e));
                    }
                }
            }
        }

        if pipes.is_empty() == false {
            ctx.raise(Event::Accepted(pipes));
        }
    }

    fn create_pipe(&self, stream: TcpStream) -> Box<pipe::Pipe> {
        let stub = TcpPipeStub::new(stream, self.recv_max_size);

        box AsyncPipe::new(stub, self.proto_ids)
    }
}

impl acceptor::Acceptor for TcpAcceptor {
    fn ready(&mut self, ctx: &mut Context, events: mio::Ready) {
        if events.is_readable() {
            self.accept(ctx);
        }
    }

    fn open(&mut self, ctx: &mut Context) {
        ctx.register(&self.listener, mio::Ready::readable(), mio::PollOpt::edge());
        ctx.raise(Event::Opened);
    }

    fn close(&mut self, ctx: &mut Context) {
        ctx.deregister(&self.listener);
        ctx.raise(Event::Closed);
    }
}
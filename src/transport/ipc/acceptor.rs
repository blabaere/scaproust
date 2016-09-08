// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;

use mio;

use mio_uds::{UnixListener, UnixStream};

use transport::*;
use transport::acceptor::*;
use transport::async::AsyncPipe;
use super::stub::IpcPipeStub;

pub struct IpcAcceptor {
    listener: UnixListener,
    proto_ids: (u16, u16)
}

impl IpcAcceptor {

    pub fn new(l: UnixListener, pids: (u16, u16)) -> IpcAcceptor {
        IpcAcceptor {
            listener: l,
            proto_ids: pids
        }
    }

    fn accept(&mut self, ctx: &mut Context) {
        let mut pipes = Vec::new();

        loop {
            match self.listener.accept() {
                Ok(Some((stream, _))) => {
                    let pipe = self.create_pipe(stream);

                    pipes.push(pipe);
                },
                Ok(None) => {
                    break;
                }
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

    fn create_pipe(&self, stream: UnixStream) -> Box<pipe::Pipe> {
        let pids = self.proto_ids;
        let stub = IpcPipeStub::new(stream);

        box AsyncPipe::new(stub, pids)
    }
}

impl acceptor::Acceptor for IpcAcceptor {
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
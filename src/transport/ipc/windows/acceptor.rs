// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use mio;

use mio_named_pipes::NamedPipe;

use transport::*;
use transport::acceptor::*;
use transport::async::AsyncPipe;
use super::stub::IpcPipeStub;

pub struct IpcAcceptor {
    addr: String,
    proto_ids: (u16, u16),
    recv_max_size: u64
}

impl IpcAcceptor {

    pub fn new(a: String, pids: (u16, u16), recv_max_size: u64) -> IpcAcceptor {
        IpcAcceptor {
            addr: a,
            proto_ids: pids,
            recv_max_size: recv_max_size
        }
    }

    fn connect(&mut self, ctx: &mut Context) {
        let name = format!(r"\\.\pipe\my-pipe-{}", self.addr);
        info!("creating ipc acceptor named pipe: {}", &name);
        //let name = format!(r"\\.\{}", self.addr);

        match NamedPipe::new(&name) {
            Ok(named_pipe) => {
                let pipe = self.create_pipe(named_pipe);
                let pipes = vec!(pipe);
                let evt = Event::Accepted(pipes);

                ctx.raise(evt);
            },
            Err(e) => {
                let evt = Event::Error(e);

                ctx.raise(evt);
            }
        }
    }

    fn create_pipe(&self, named_pipe: NamedPipe) -> Box<pipe::Pipe> {
        let stub = IpcPipeStub::new(named_pipe, self.recv_max_size);

        box AsyncPipe::new(stub, self.proto_ids)
    }
}

impl acceptor::Acceptor for IpcAcceptor {
    fn ready(&mut self, _: &mut Context, _: mio::Ready) {
    }

    fn open(&mut self, ctx: &mut Context) {
        info!("opening ipc acceptor");
        ctx.raise(Event::Opened);

        self.connect(ctx);
    }

    fn close(&mut self, ctx: &mut Context) {

        // TODO find a way to drop the created pipe
        ctx.raise(Event::Closed);
    }
}

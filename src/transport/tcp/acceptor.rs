// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::io;

use mio;
use mio::tcp::{TcpListener, TcpStream, Shutdown};

use transport::{Endpoint, AcceptorCmd, AcceptorEvt, Context, PipeCmd, PipeEvt};
use transport::tcp::step::*;
use transport::stream::*;

pub struct Acceptor {
    listener: TcpListener,
    pids: (u16, u16)
}

impl Acceptor {
    pub fn new(listener: TcpListener, pids: (u16, u16)) -> Acceptor {
        Acceptor {
            listener: listener,
            pids: pids
        }
    }

    fn open(&mut self, ctx: &mut Context<AcceptorEvt>) {
        ctx.register(&self.listener, mio::EventSet::readable(), mio::PollOpt::edge()).expect("Acceptor.open failed");
        ctx.raise(AcceptorEvt::Opened);
    }

    fn close(&mut self, ctx: &mut Context<AcceptorEvt>) {
        ctx.deregister(&self.listener);
        ctx.raise(AcceptorEvt::Closed);
    }

    fn accept(&mut self, ctx: &mut Context<AcceptorEvt>) {
        let mut pipes = Vec::new();

        loop {
            match self.listener.accept() {
                Ok(None) => break,
                Ok(Some((stream, _))) => {
                    let pipe = self.create_pipe(stream);

                    pipes.push(pipe);
                },
                Err(e) => {

                }
            }
        }

        if pipes.len() > 0 {
            ctx.raise(AcceptorEvt::Accepted(pipes));
        }
    }

    fn create_pipe(&self, stream: TcpStream) -> Box<Endpoint<PipeCmd, PipeEvt>> {
        box Pipe::new(TcpStepStream::new(stream), self.pids)
    }
}

impl Endpoint<AcceptorCmd, AcceptorEvt> for Acceptor {
    fn ready(&mut self, ctx: &mut Context<AcceptorEvt>, events: mio::EventSet) {
        if events.is_readable() {
            self.accept(ctx);
        }
    }
    fn process(&mut self, ctx: &mut Context<AcceptorEvt>, cmd: AcceptorCmd) {
        match cmd {
            AcceptorCmd::Open => self.open(ctx),
            AcceptorCmd::Close => self.close(ctx)
        }
    }
}

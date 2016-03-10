// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::HashMap;

use mio;

use EventLoop;
use pipe::Pipe;
use super::with_notify::WithNotify;

pub trait WithPipes : WithNotify {
    fn get_pipes(&self) -> &HashMap<mio::Token, Pipe>;
    fn get_pipes_mut(&mut self) -> &mut HashMap<mio::Token, Pipe>;

    fn get_pipe(&self, tok: &mio::Token) -> Option<&Pipe> {
        self.get_pipes().get(tok)
    }

    fn get_pipe_mut(&mut self, tok: &mio::Token) -> Option<&mut Pipe> {
        self.get_pipes_mut().get_mut(tok)
    }

    fn destroy_pipes(&mut self, event_loop: &mut EventLoop) {
        let _: Vec<_> = self.get_pipes_mut().drain().map(|(_, mut p)| p.close(event_loop)).collect();
    }
}

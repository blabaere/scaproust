// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use mio;

use transport::pipe::Pipe;

pub struct Excl {
    token: Option<mio::Token>,
    pipe: Option<Pipe>,
    active: bool
}

impl Excl {

    pub fn new() -> Excl {
        Excl {
            token: None,
            pipe: None,
            active: false
        }
    }

    pub fn add(&mut self, tok: mio::Token, pipe: Pipe) -> bool {
        if self.token.is_some() {
            false
        } else {
            self.token = Some(tok);
            self.pipe = Some(pipe);
            true
        }
    }

    pub fn remove(&mut self, tok: mio::Token) -> Option<Pipe> {
        if self.token.is_none() {
            return None;
        }

        if self.token != Some(tok) {
            return None;
        }

        self.token = None;
        self.active = false;

        self.pipe.take()
    }

    pub fn activate(&mut self, tok: mio::Token) {
        self.active = if self.token == Some(tok) {
            true
        } else {
            self.active
        };
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn get(&mut self, tok: mio::Token) -> Option<&mut Pipe> {
        if self.token == Some(tok) {
            self.pipe.as_mut()
        } else {
            None
        }
    }

    pub fn get_active_mut(&mut self) -> Option<&mut Pipe> {
        if self.active {
            self.pipe.as_mut()
        } else {
            None
        }
    }

    pub fn get_active(&self) -> Option<&Pipe> {
        if self.active {
            self.pipe.as_ref()
        } else {
            None
        }
    }

    pub fn get_pipe(&mut self) -> Option<&mut Pipe> {
        self.pipe.as_mut()
    }
}

impl Default for Excl {
    fn default() -> Self {
        Excl::new()
    }
}

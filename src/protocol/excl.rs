// Copyright (c) 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the MIT license LICENSE or <http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according to those terms.

use mio;

use pipe::*;

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

    pub fn get<'a>(&'a mut self, tok: mio::Token) -> Option<&'a mut Pipe> {
        if self.token == Some(tok) {
            self.pipe.as_mut()
        } else {
            None
        }
    }

    pub fn get_active<'a>(&'a mut self) -> Option<&'a mut Pipe> {
        if self.active {
            self.pipe.as_mut()
        } else {
            None
        }
    }
}
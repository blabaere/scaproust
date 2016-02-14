// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::HashMap;

use mio;

use pipe::*;
use super::with_notify::WithNotify;

pub trait WithPipes : WithNotify {
    fn get_pipes<'a>(&'a self) -> &'a HashMap<mio::Token, Pipe>;
    fn get_pipes_mut<'a>(&'a mut self) -> &'a mut HashMap<mio::Token, Pipe>;

    fn get_pipe<'a>(&'a mut self, tok: &mio::Token) -> Option<&'a mut Pipe> {
        self.get_pipes_mut().get_mut(tok)
    }
}

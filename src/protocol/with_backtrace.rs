// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

pub trait WithBacktrace {

    fn get_backtrace<'a>(&'a self) -> &'a Vec<u8>;
    fn get_backtrace_mut<'a>(&'a mut self) -> &'a mut Vec<u8>;

    fn backtrace<'a>(&'a self) -> &'a [u8] {
        &self.get_backtrace()
    }

    fn set_backtrace(&mut self, backtrace: &[u8]) {
        self.get_backtrace_mut().clear();
        self.get_backtrace_mut().extend_from_slice(backtrace);
    }

    fn clear_backtrace(&mut self) {
        self.get_backtrace_mut().clear();
    }
}
// Copyright (c) 2015-2017 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone)]
pub struct Sequence {
    value: Rc<Cell<usize>>
}

impl Sequence {
    pub fn new() -> Sequence {
        Sequence { value: Rc::new(Cell::new(0)) }
    }

    pub fn next(&self) -> usize {
        let id = self.value.get();

        self.value.set(id + 1);
        id
    }
}

impl Default for Sequence {
    fn default() -> Self {
        Sequence::new()
    }
}

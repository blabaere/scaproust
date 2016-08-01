// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

pub struct Message {
    header: Vec<u8>,
    body: Vec<u8>
}

impl Message {
    fn new() -> Message {
        Message {
            header: Vec::new(),
            body: Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn can_compare_array_slices() {
    }
}
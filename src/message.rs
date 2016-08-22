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
    pub fn new() -> Message {
        Message {
            header: Vec::new(),
            body: Vec::new()
        }
    }

    pub fn from_header_and_body(header: Vec<u8>, body: Vec<u8>) -> Message {
        Message {
            header: header,
            body: body
        }
    }

    pub fn len(&self) -> usize {
        self.header.len() + self.body.len()
    }

    pub fn get_header(&self) -> &[u8] {
        &self.header
    }

    pub fn get_body(&self) -> &[u8] {
        &self.body
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn can_compare_array_slices() {
    }
}
// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

extern crate scaproust;

use scaproust::*;

#[test]
fn can_create_socket() {
    let mut session = SessionBuilder::build().unwrap();
    let mut socket = session.create_socket::<Push>().unwrap();
    let ep = socket.connect("url");
}

pub struct Push {
    x: i32
}
impl Protocol for Push {
    fn do_it_bob(&self) -> u8 { 0 }
}

impl From<i32> for Push {
    fn from(value: i32) -> Push {
        Push {x: value}
    }
}

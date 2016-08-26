// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

#![crate_name = "scaproust"]
#![doc(html_root_url = "https://blabaere.github.io/scaproust/")]

#![feature(box_syntax)]
#![feature(unboxed_closures)]
#![feature(fnbox)]
#![feature(associated_type_defaults)]

//#![feature(plugin)]
//#![plugin(clippy)]
//#![allow(boxed_local)]
//#![allow(bool_comparison)]
//#![allow(explicit_iter_loop)]

#[macro_use]
extern crate log;
extern crate byteorder;
extern crate mio;
extern crate time;

mod sequence;
mod core;
mod ctrl;
mod facade;
mod transport;
mod io_error;

pub use facade::session::SessionBuilder;
pub use facade::session::Session;
pub use facade::socket::Socket;

pub use core::socket::Reply as SocketReply;
pub use core::protocol::Protocol;
pub use core::endpoint::EndpointId;
pub use core::endpoint::Pipe;
pub use core::network::Network;
pub use core::message::Message;

#[cfg(test)]
mod tests {
    struct Editable {
        x: usize
    }
    struct Editor {
        y: usize
    }
    struct Outter {
        editable: Editable,
        editor: Editor
    }
    impl Editable {
        fn edit(&mut self) { self.x += 1; }
    }
    impl Editor {
        fn edit(&mut self, editable: &mut Editable) { 
            self.y += 1; 
            editable.edit();
        }
    }
    impl Outter {
        fn test(&mut self) {
            self.editor.edit(&mut self.editable);
        }
    }

    #[test]
    fn can_pass_mutable_field_ref_to_other_field() {
        let mut master = Outter {
            editable: Editable { x: 0 },
            editor: Editor { y: 0 },
        };

        master.test();
    }
}
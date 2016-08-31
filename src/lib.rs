// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

#![crate_name = "scaproust"]
#![doc(html_root_url = "https://blabaere.github.io/scaproust/")]

#![feature(box_syntax)]
#![feature(fnbox)]
#![feature(stmt_expr_attributes)]

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

pub mod core;
pub mod proto;
mod reactor;
mod facade;
mod transport;

mod sequence;
mod io_error;

pub use facade::session::SessionBuilder;
pub use facade::session::Session;
pub use facade::socket::Socket;

pub use proto::pair::Pair;
pub use proto::publ::Pub;
pub use proto::sub::Sub;
pub use proto::push::Push;
pub use proto::pull::Pull;

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
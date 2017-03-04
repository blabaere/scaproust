// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

use std::collections::HashMap;
use std::rc::Rc;

use core::{BuildIdHasher, EndpointId, Message};
use core::endpoint::Pipe;
use core::context::Context;

pub struct PipeCollection {
    pipes: HashMap<EndpointId, Pipe, BuildIdHasher>
}

impl Default for PipeCollection {
     fn default() -> PipeCollection {
         PipeCollection::new()
     }
}

impl PipeCollection {
    pub fn new() -> PipeCollection {
        PipeCollection {
            pipes: HashMap::default()
        }
    }

    pub fn insert(&mut self, id: EndpointId, pipe: Pipe) -> Option<Pipe> {
        self.pipes.insert(id, pipe)
    }

    pub fn remove(&mut self, id: &EndpointId) -> Option<Pipe> {
        self.pipes.remove(id)
    }

    pub fn get_mut(&mut self, id: &EndpointId) -> Option<&mut Pipe> {
        self.pipes.get_mut(id)
    }

    pub fn send_to(&mut self, ctx: &mut Context, msg: Rc<Message>, eid: EndpointId) -> Option<EndpointId> {
        self.pipes.get_mut(&eid).map_or(None, |pipe| {
            pipe.send(ctx, msg); 
            Some(eid)
        })
    }

    pub fn recv_from(&mut self, ctx: &mut Context, eid: EndpointId) -> Option<EndpointId> {
        self.pipes.get_mut(&eid).map_or(None, |pipe| {
            pipe.recv(ctx); 
            Some(eid)
        })
    }

    pub fn close_all(&mut self, ctx: &mut Context) {
        for (_, pipe) in self.pipes.drain() {
            pipe.close(ctx);
        }
    }

}
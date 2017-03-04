// Copyright (c) 2015-2016 Contributors as noted in the AUTHORS file.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

pub mod broadcast {

    use std::collections::HashSet;
    use std::rc::Rc;

    use core::{EndpointId, Message};
    use core::context::Context;
    use proto::pipes::PipeCollection;

    pub fn send_to_all(
        bc: &mut HashSet<EndpointId>, 
        pipes: &mut PipeCollection,
        ctx: &mut Context, 
        msg: Rc<Message>) {

        for id in bc.drain() {
            pipes.get_mut(&id).map(|pipe| pipe.send(ctx, msg.clone()));
        }
    }
    pub fn send_to_all_except(
        bc: &mut HashSet<EndpointId>, 
        pipes: &mut PipeCollection,
        ctx: &mut Context, 
        msg: Rc<Message>, 
        except: EndpointId) {

        for id in bc.drain().filter(|x| *x != except) {
            pipes.get_mut(&id).map(|pipe| pipe.send(ctx, msg.clone()));
        }
        bc.insert(except);
    }
}

pub mod fair_queue {

    use core::EndpointId;
    use core::context::Context;
    use proto::priolist::Priolist;
    use proto::pipes::PipeCollection;

    pub fn recv(fq: &mut Priolist, pipes: &mut PipeCollection, ctx: &mut Context) -> Option<EndpointId> {
        fq.pop().map_or(None, |eid| pipes.recv_from(ctx, eid))
    }
}

pub mod load_balancing {

    use std::rc::Rc;

    use core::{EndpointId, Message};
    use core::context::Context;
    use proto::priolist::Priolist;
    use proto::pipes::PipeCollection;

    pub fn send(
        lb: &mut Priolist, 
        pipes: &mut PipeCollection, 
        ctx: &mut Context, 
        msg: Rc<Message>) -> Option<EndpointId> {
        lb.pop().map_or(None, |eid| pipes.send_to(ctx, msg, eid))
    }
    
}
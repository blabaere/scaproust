// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.

/*
WARNING:  
Priolist can not own pipes, since fq & lb deal only with one operation.
Bus protocol, for example uses both fq and broadcast.

Since send and recv are mutable operations, the item type must be either EndpointId.
This would cost a HashMap lookup for each operation.

Or the usual shared mutability recipe: Rc<RefCell<Pipe>>.
This would cost a borrow mut on each operation, and memory fragmentation.

Or maybe FairQueue, LoadBalancing & Broadcast should each own an Rc<RefCell<PrioList>> ?
NO !!! broadcast is not priority based.

FairQueue     --> PrioList --> Rc<RefCell<HashMap<Eid, ActivablePipe>>>
LoadBalancing --> PrioList --> Rc<RefCell<HashMap<Eid, ActivablePipe>>>
Broadcast     ---------------> Rc<RefCell<HashMap<Eid, ActivablePipe>>>

but then, where should add & remove operations be located ?

struct ActivablePipe {
    pipe: Pipe,
    active: bool // nein, active is relative to operation
}

PROTOCOL    |   SEND          |   RECV
-------------------------------------------
bus         |   broadcast     |   fairqueue
pair        |   single        |   single
publisher   |   broadcast     |   XXX 
pull        |   XXX           |   fairqueue
push        |   loadbalancer  |   XXX 
reply       |   target        |   fairqueue
request     |   loadbalancer  |   target
respondent  |   target        |   fairqueue
subscribe   |   XXX           |   fairqueue 
survey      |   broadcast     |   fairqueue 

Priolist needs to support:
 - insert (token, pipe?, priority): called once per pipe 
 - remove (token): called once per pipe 
 - activate (token): called each time a pipe is ready to write/read
 - next() -> token: called each time a msg is sent/received
*/

/* functions spec
### INSERT
 - store the item
 - assign the item a priority

### ACTIVATE
 - mark the item as active 
 - if there was no current, it becomes current
 - if the current had lower priority, it becomes current

### NEXT 
 - If there is no current, abort
 - Deactivate the current item
 - Select 'new' current amongst active items
 - Return 'old' current

### REMOVE 
 - Remove from storage
 - if removed item was current, select another item to be the current

*/

use std::ops::Range;

use core::EndpointId;

pub struct Priolist {
    items: Vec<Item>,
    current: Option<(usize, u8)>
}

struct Item {
    value: EndpointId,
    priority: u8,
    active: bool,
}

impl Priolist {

    pub fn new() -> Priolist {
        Priolist {
            items: Vec::new(),
            current: None
        }
    }

    pub fn insert(&mut self, id: EndpointId, prio: u8) {
        self.items.push(Item::new(id, prio))
    }

    pub fn remove(&mut self, id: &EndpointId) {
        if let Some(index) = self.find_by_id_in_all(id) {
            self.remove_at_index(index);
        }
    }

    fn remove_at_index(&mut self, index: usize) {
        let item = self.items.swap_remove(index);
        let priority = item.priority;

        if self.current == Some((index, priority)) {
            self.compute_next(index, priority);
        }
    }

    pub fn activate(&mut self, id: &EndpointId) {
        if let Some(index) = self.find_by_id_in_all(id) {
            self.activate_at_index(index);
        }
    }

    fn activate_at_index(&mut self, index: usize) {
        if self.is_index_active(index) {
            return;
        }

        let priority = self.items[index].priority;

        self.set_index_active(index, true);

        if let Some((cur_idx, cur_prio)) = self.current.take() {
            if priority < cur_prio {
                self.set_current(index, priority);
            } else {
                self.set_current(cur_idx, cur_prio);
            }
        } else {
            self.set_current(index, priority);
        }
    }

    fn is_index_active(&self, index: usize) -> bool {
        self.items[index].active
    }

    fn set_index_active(&mut self, index: usize, active: bool) {
        self.items[index].active = active;
    }

    pub fn pop(&mut self) -> Option<EndpointId> {
        if let Some((index, priority)) = self.current.take() {
            self.set_index_active(index, false);
            self.compute_next(index, priority);

            Some(self.items[index].value)
        } else {
            None
        }
    }

    fn compute_next(&mut self, pivot: usize, priority: u8) {
        if let Some(index) = self.find(|x| x.active && x.priority == priority, pivot..self.len()) {
            return self.set_current(index, priority);
        }

        if let Some(index) = self.find(|x| x.active && x.priority == priority, 0..pivot) {
            return self.set_current(index, priority);
        }

        let lower_priority = priority + 1;
        let lower_priorities = lower_priority..17;
        for prio in lower_priorities {
            if let Some(index) = self.find(|x| x.active && x.priority == prio, self.all()) {
                return self.set_current(index, prio);
            }
        }

        self.unset_current();
    }

    fn set_current(&mut self, index: usize, priority: u8) {
        self.current = Some((index, priority));
    }

    fn unset_current(&mut self) {
        self.current = None;
    }

    fn len(&self) -> usize { self.items.len() }
    //pub fn is_empty(&self) -> bool { self.len() == 0 }

    fn all(&self) -> Range<usize> {
        0..self.len()
    }

    fn find_by_id_in_all(&self, id: &EndpointId) -> Option<usize> {
        self.find_by_id(id, self.all())
    }

    fn find_by_id(&self, id: &EndpointId, range: Range<usize>) -> Option<usize> {
        self.find(|x| x.value == *id, range)
    }

    fn find<F>(&self, predicate: F, range: Range<usize>) -> Option<usize> 
    where F : Fn(&Item) -> bool {

        for i in range {
            let item = &self.items[i];

            if predicate(item) {
                return Some(i);
            }
        }

        None
    }
}

impl Item {
    fn new(id: EndpointId, prio: u8) -> Item {
        Item {
            value: id,
            priority: prio,
            active: false
        }
    }
}

#[cfg(test)]
mod tests {

    use core::EndpointId;

    use super::Priolist;

    #[test]
    fn insert_does_not_activate() {
        let mut priolist = Priolist::new();
        let eid = EndpointId::from(0);

        priolist.insert(eid, 8);
        assert!(priolist.pop().is_none());
    }

    #[test]
    fn activate_makes_next_available() {
        let mut priolist = Priolist::new();
        let eid = EndpointId::from(0);

        priolist.insert(eid, 8);
        priolist.activate(&eid);
        assert_eq!(Some(eid), priolist.pop());
    }

    #[test]
    fn activate_does_not_change_existing_next() {
        let mut priolist = Priolist::new();
        let first = EndpointId::from(0);
        let second = EndpointId::from(1);

        priolist.insert(first, 8);
        priolist.insert(second, 8);
        priolist.activate(&first);
        priolist.activate(&second);
        assert_eq!(Some(first), priolist.pop());
    }

    #[test]
    fn next_can_move_forward() {
        let mut priolist = Priolist::new();
        let first = EndpointId::from(0);
        let second = EndpointId::from(1);

        priolist.insert(first, 8);
        priolist.insert(second, 8);
        priolist.activate(&first);
        priolist.activate(&second);
        assert_eq!(Some(first), priolist.pop());
        assert_eq!(Some(second), priolist.pop());
    }

    #[test]
    fn next_can_wrap() {
        let mut priolist = Priolist::new();
        let one = EndpointId::from(0);
        let two = EndpointId::from(1);
        let three = EndpointId::from(2);
        let four = EndpointId::from(3);

        priolist.insert(one, 8);
        priolist.insert(two, 8);
        priolist.insert(three, 8);
        priolist.insert(four, 8);

        priolist.activate(&three);
        priolist.activate(&four);
        priolist.activate(&one);
        priolist.activate(&two);
        assert_eq!(Some(three), priolist.pop());
        assert_eq!(Some(four), priolist.pop());
        assert_eq!(Some(one), priolist.pop());
        assert_eq!(Some(two), priolist.pop());
    }

    #[test]
    fn next_deactivates() {
        let mut priolist = Priolist::new();
        let eid = EndpointId::from(0);

        priolist.insert(eid, 8);
        priolist.activate(&eid);
        assert_eq!(Some(eid), priolist.pop());
        assert_eq!(None, priolist.pop());
    }

    #[test]
    fn next_can_skip_lower_priorities() {
        let mut priolist = Priolist::new();
        let one = EndpointId::from(0);
        let two = EndpointId::from(1);
        let three = EndpointId::from(2);
        let four = EndpointId::from(3);

        priolist.insert(one, 0);
        priolist.insert(two, 8);
        priolist.insert(three, 0);
        priolist.insert(four, 8);

        priolist.activate(&three);
        priolist.activate(&four);
        priolist.activate(&one);
        priolist.activate(&two);

        assert_eq!(Some(three), priolist.pop());
        assert_eq!(Some(one), priolist.pop());
    }

    #[test]
    fn remove_current_can_make_next_unavailable() {
        let mut priolist = Priolist::new();
        let one = EndpointId::from(0);
        let two = EndpointId::from(1);
        let three = EndpointId::from(2);
        let four = EndpointId::from(3);

        priolist.insert(one, 0);
        priolist.insert(two, 8);
        priolist.insert(three, 0);
        priolist.insert(four, 8);

        priolist.activate(&three);
        priolist.remove(&three);
        assert_eq!(None, priolist.pop());
    }

    #[test]
    fn remove_current_can_move_forward() {
        let mut priolist = Priolist::new();
        let one = EndpointId::from(0);
        let two = EndpointId::from(1);
        let three = EndpointId::from(2);
        let four = EndpointId::from(3);

        priolist.insert(one, 8);
        priolist.insert(two, 8);
        priolist.insert(three, 8);
        priolist.insert(four, 8);

        priolist.activate(&three);
        priolist.activate(&four);
        priolist.remove(&three);
        assert_eq!(Some(four), priolist.pop());
    }

    #[test]
    fn remove_current_can_wrap() {
        let mut priolist = Priolist::new();
        let one = EndpointId::from(0);
        let two = EndpointId::from(1);
        let three = EndpointId::from(2);
        let four = EndpointId::from(3);

        priolist.insert(one, 8);
        priolist.insert(two, 8);
        priolist.insert(three, 8);
        priolist.insert(four, 8);

        priolist.activate(&three);
        priolist.activate(&two);
        priolist.remove(&three);
        assert_eq!(Some(two), priolist.pop());
    }

    #[test]
    fn remove_can_skip_lower_priorities() {
        let mut priolist = Priolist::new();
        let one = EndpointId::from(0);
        let two = EndpointId::from(1);
        let three = EndpointId::from(2);
        let four = EndpointId::from(3);

        priolist.insert(one, 0);
        priolist.insert(two, 8);
        priolist.insert(three, 0);
        priolist.insert(four, 8);

        priolist.activate(&three);
        priolist.activate(&one);
        priolist.activate(&two);
        priolist.activate(&four);
        priolist.remove(&three);
        assert_eq!(Some(one), priolist.pop());
    }

    #[test]
    fn activate_higher_priority_changes_next() {
        let mut priolist = Priolist::new();
        let one = EndpointId::from(0);
        let two = EndpointId::from(1);
        let three = EndpointId::from(2);
        let four = EndpointId::from(3);

        priolist.insert(one, 8);
        priolist.insert(two, 4);
        priolist.insert(three, 0);
        priolist.insert(four, 8);

        priolist.activate(&one);
        priolist.activate(&four);
        assert_eq!(Some(one), priolist.pop());

        priolist.activate(&two);
        assert_eq!(Some(two), priolist.pop());

        priolist.activate(&three);
        assert_eq!(Some(three), priolist.pop());
    }
}
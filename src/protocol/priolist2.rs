/*
WARNING:  
fq & lb must not own the pipes, since they deal only one side of endpoint.
Bus, for example uses fq and broadcast.
So they should store some references, or Rc ?

lb & fq needs to support:
 - add (pipe/tok, priority): called once per pipe 
 - remove (token): called once per pipe 
 - activate (token): called each time a pipe is ready to write/read
 - move next(): called each time a msg is sent/received
 - get() -> tok: call when send/recv is requested
*/

/* functions spec
### ADD assign the item a priority

### ACTIVATE
 - make the item part of the active items subset
 - if there was no current, it becomes current
 - if the current had lower priority, it becomes current

### ADVANCE 
 - if current is to be released, remove it from the active items subset
 - select the new current amongst active items

### GET returns the current item if any

### REMOVE 
 - remove the priority association
 - if active, remove the item from active items
 - if current, select another item to be the current

*/
use std::ops::Range;

use mio;

pub struct PrioList {
    items: Vec<PrioListItem>,
    current: Option<usize>
}

impl PrioList {
    pub fn new() -> PrioList {
        PrioList {
            items: Vec::new(),
            current: None
        }
    }

    pub fn get(&self) -> Option<mio::Token> {
        self.current.map(|i| self.items[i].token)
    }

    fn set(&mut self, index: usize) {
        self.current = Some(index);
    }

    fn unset(&mut self) {
        self.current = None;
    }

    fn full_range(&self) -> Range<usize> {
        0..self.items.len()
    }

    pub fn insert(&mut self, tok: mio::Token, priority: u8) {
        self.items.push(PrioListItem::new(tok, priority));
    }

    pub fn remove(&mut self, tok: mio::Token) {
        let all = self.full_range();
        if let Some(index) = self.find_item_index(all, &|item| item.token == tok) {
            self.remove_index(index);
        }
    }

    fn remove_index(&mut self, index: usize) {
        if let Some(current) = self.current {
            if index == current {
                self.deactivate_index_and_advance(index);
                if let Some(new_current) = self.current {
                    self.remove_index_that_is_not_current(index, new_current);
                }
            } else {
                self.remove_index_that_is_not_current(index, current);
            }
        } else {
            self.items.swap_remove(index);
        }
    }

    fn remove_index_that_is_not_current(&mut self, index: usize, current: usize) {
        if index < current {
            self.items.swap(index, current);
            self.items.swap_remove(current);
            self.set(index);
        } else { // index > current
            self.items.swap_remove(index);
        }
    }

    pub fn activate(&mut self, tok: mio::Token) {
        let activable_index = self.find_activable_index(tok);

        if let Some(index) = activable_index {
            self.activate_index(index);
        }
    }

    fn activate_index(&mut self, index: usize) {
        self.items[index].active = true;

        if let Some(current) = self.current {
            if self.items[index].priority < self.items[current].priority {
                self.set(index);
            }
        } else {
            self.set(index);
        }
    }

    pub fn advance(&mut self) {
        if let Some(index) = self.current {
            let priority = self.items[index].priority;
            if let Some(i) = self.find_active_index_after(index, priority) {
                self.set(i);
            } else if let Some(i) = self.find_active_index_before(index, priority) {
                self.set(i);
            }
        }
    }

    fn find_activable_index(&self, tok: mio::Token) -> Option<usize> {
        let all = self.full_range();
        self.find_item_index(all, &|item|item.token == tok && item.active == false)
    }

    fn find_active_index_after(&self, pivot: usize, priority: u8) -> Option<usize> {
        let from = pivot + 1;
        let to = self.items.len();

        self.find_active_index(from..to, priority)
    }

    fn find_active_index_before(&self, pivot: usize, priority: u8) -> Option<usize> {
        self.find_active_index(0..pivot, priority)
    }

    fn find_active_index(&self, range: Range<usize>, priority: u8) -> Option<usize> {
        self.find_item_index(range, &|item| item.active && item.priority == priority)
    }

    fn find_item_index<P>(&self, range: Range<usize>, predicate: &P) -> Option<usize> where P: Fn(&PrioListItem) -> bool {
        for i in range {
            if predicate(&self.items[i]) {
                return Some(i);
            }
        }

        None
    }

    pub fn deactivate_and_advance(&mut self) {
        if let Some(index) = self.current {
            self.deactivate_index_and_advance(index)
        } else {
            self.advance()
        }
    }

    fn deactivate_index_and_advance(&mut self, index: usize) {
        self.items[index].active = false;
        self.unset();

        let priority = self.items[index].priority;
        if let Some(i) = self.find_active_index_after(index, priority) {
            self.set(i);
        } else if let Some(i) = self.find_active_index_before(index, priority) {
            self.set(i);
        } else if priority < 16 {
            self.advance_through_priorities(priority + 1);
        }
    }

    fn advance_through_priorities(&mut self, from: u8) {
        for priority in from..16 {
            let all = self.full_range();
            if let Some(i) = self.find_active_index(all, priority) {
                self.set(i);
                break;
            }
        }
    }
}

struct PrioListItem {
    token: mio::Token,
    priority: u8,
    active: bool
}

impl PrioListItem {
    fn new(token: mio::Token, priority: u8) -> PrioListItem {
        PrioListItem {
            token: token,
            priority: priority,
            active: false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PrioList;

    use mio;
    
    #[test]
    fn when_created_list_is_empty() {
        let list = PrioList::new();
        let current = list.get();

        assert!(current.is_none());
    }

    #[test]
    fn when_item_is_added_it_is_not_active() {
        let mut list = PrioList::new();
        let token = mio::Token(1);

        list.insert(token, 8);

        assert!(list.get().is_none());
    }

    #[test]
    fn when_2nd_item_is_added_it_is_not_active() {
        let mut list = PrioList::new();

        list.insert(mio::Token(1), 8);
        list.insert(mio::Token(2), 8);

        assert!(list.get().is_none());
    }

    #[test]
    fn find_activable_index_works() {
        let mut list = PrioList::new();

        list.insert(mio::Token(1), 8);
        assert_eq!(Some(0), list.find_activable_index(mio::Token(1)));
        list.insert(mio::Token(2), 8);
        assert_eq!(Some(0), list.find_activable_index(mio::Token(1)));
        assert_eq!(Some(1), list.find_activable_index(mio::Token(2)));
        assert_eq!(None, list.find_activable_index(mio::Token(3)));
    }

    #[test]
    fn when_single_item_is_activated_it_becomes_current() {
        let mut list = PrioList::new();
        let token = mio::Token(1);

        list.insert(token, 8);
        list.activate(token);
        assert_eq!(Some(token), list.get());
    }

    #[test]
    fn when_item_zero_of_two_is_activated_it_becomes_current() {
        let mut list = PrioList::new();
        let token = mio::Token(1);

        list.insert(token, 8);
        list.insert(mio::Token(2), 8);
        list.insert(mio::Token(3), 8);

        list.activate(token);
        assert_eq!(Some(token), list.get());
    }

    #[test]
    fn when_item_one_of_two_is_activated_it_becomes_current() {
        let mut list = PrioList::new();
        let token = mio::Token(2);

        list.insert(mio::Token(1), 8);
        list.insert(token, 8);
        list.activate(token);
        assert_eq!(Some(token), list.get());
        list.insert(mio::Token(3), 8);
        list.activate(token);
    }

    #[test]
    fn when_activating_another_item_with_same_priority_it_does_not_become_current() {
        let mut list = PrioList::new();

        list.insert(mio::Token(10), 8);
        list.insert(mio::Token(20), 8);
        list.activate(mio::Token(20));
        assert_eq!(Some(mio::Token(20)), list.get());
        list.activate(mio::Token(10));
        assert_eq!(Some(mio::Token(20)), list.get());
    }

    #[test]
    fn when_activating_another_item_with_higher_priority_it_becomes_current() {
        let mut list = PrioList::new();
        
        list.insert(mio::Token(10), 8);
        list.insert(mio::Token(20), 2);

        list.activate(mio::Token(10));
        list.activate(mio::Token(20));
        assert_eq!(Some(mio::Token(20)), list.get());
    }

    #[test]
    fn advance_empty_list_does_nothing() {
        let mut list = PrioList::new();

        list.advance();
        assert!(list.get().is_none());
    }

    #[test]
    fn advance_whith_single_item_does_nothing() {
        let mut list = PrioList::new();

        list.insert(mio::Token(10), 8);
        list.advance();
        assert!(list.get().is_none());
    }

    #[test]
    fn advance_with_single_active_item_loops() {
        let mut list = PrioList::new();

        list.insert(mio::Token(10), 8);
        list.activate(mio::Token(10));
        list.advance();
        assert_eq!(Some(mio::Token(10)), list.get());
    }

    #[test]
    fn find_active_index_after_works() {
        let mut list = PrioList::new();
        
        list.insert(mio::Token(10), 8);
        list.insert(mio::Token(20), 8);
        list.insert(mio::Token(30), 8);

        assert_eq!(None, list.find_active_index_after(0, 8));
        assert_eq!(None, list.find_active_index_after(1, 8));
        assert_eq!(None, list.find_active_index_after(2, 8));

        list.activate(mio::Token(10));
        assert_eq!(None, list.find_active_index_after(0, 8));
        assert_eq!(None, list.find_active_index_after(1, 8));
        assert_eq!(None, list.find_active_index_after(2, 8));

        list.activate(mio::Token(20));
        assert_eq!(Some(1), list.find_active_index_after(0, 8));
        assert_eq!(None, list.find_active_index_after(1, 8));
        assert_eq!(None, list.find_active_index_after(2, 8));

        list.activate(mio::Token(30));
        assert_eq!(Some(1), list.find_active_index_after(0, 8));
        assert_eq!(Some(2), list.find_active_index_after(1, 8));
        assert_eq!(None, list.find_active_index_after(2, 8));
    }

    #[test]
    fn find_active_index_before_works() {
        let mut list = PrioList::new();
        
        list.insert(mio::Token(10), 8);
        list.insert(mio::Token(20), 8);
        list.insert(mio::Token(30), 8);

        assert_eq!(None, list.find_active_index_before(0, 8));
        assert_eq!(None, list.find_active_index_before(1, 8));
        assert_eq!(None, list.find_active_index_before(2, 8));

        list.activate(mio::Token(30));
        assert_eq!(None, list.find_active_index_before(0, 8));
        assert_eq!(None, list.find_active_index_before(1, 8));
        assert_eq!(None, list.find_active_index_before(2, 8));

        list.activate(mio::Token(20));
        assert_eq!(None, list.find_active_index_before(0, 8));
        assert_eq!(None, list.find_active_index_before(1, 8));
        assert_eq!(Some(1), list.find_active_index_before(2, 8));

        list.activate(mio::Token(10));
        assert_eq!(None, list.find_active_index_before(0, 8));
        assert_eq!(Some(0), list.find_active_index_before(1, 8));
        assert_eq!(Some(0), list.find_active_index_before(2, 8));
    }

    #[test]
    fn advance_can_move_forward() {
        let mut list = PrioList::new();
        
        list.insert(mio::Token(10), 8);
        list.insert(mio::Token(20), 8);
        list.insert(mio::Token(30), 8);

        list.activate(mio::Token(10));
        list.activate(mio::Token(20));
        list.activate(mio::Token(30));
        list.advance();
        assert_eq!(Some(mio::Token(20)), list.get());
    }

    #[test]
    fn advance_can_skip_lower_priority() {
        let mut list = PrioList::new();
        
        list.insert(mio::Token(10), 1);
        list.insert(mio::Token(20), 9);
        list.insert(mio::Token(30), 1);

        list.activate(mio::Token(10));
        list.activate(mio::Token(20));
        list.activate(mio::Token(30));
        list.advance();
        assert_eq!(Some(mio::Token(30)), list.get());
    }

    #[test]
    fn deactivate_and_advance_can_jump_to_lower_priority() {
        let mut list = PrioList::new();
        
        list.insert(mio::Token(10), 8);
        list.insert(mio::Token(20), 10);
        list.insert(mio::Token(30), 10);

        list.activate(mio::Token(10));
        list.activate(mio::Token(20));
        list.activate(mio::Token(30));
        assert_eq!(Some(mio::Token(10)), list.get());

        list.deactivate_and_advance();
        assert_eq!(Some(mio::Token(20)), list.get());

        list.advance();
        assert_eq!(Some(mio::Token(30)), list.get());

        list.advance();
        assert_eq!(Some(mio::Token(20)), list.get());

        list.activate(mio::Token(10));
        assert_eq!(Some(mio::Token(10)), list.get());
    }

    #[test]
    fn deactivate_the_only_item_unsets_current() {
        let mut list = PrioList::new();
        
        list.insert(mio::Token(10), 8);
        list.activate(mio::Token(10));
        assert_eq!(Some(mio::Token(10)), list.get());

        list.deactivate_and_advance();
        assert_eq!(None, list.get());
    }

    #[test]
    fn for_loop_over_empty_range_does_nothing() {
        for i in 1..1 {
            panic!("{:?}", i);
        }
    }
}
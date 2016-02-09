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
 - if active, remove the item from active items if 
 - if current, select another item to be the current

*/

use mio;

/*

    fn get_client<'a>(&'a mut self, token: Token) -> &'a mut SimpleClient {
        &mut self.clients[token]
    }
*/

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

    pub fn insert(&mut self, tok: mio::Token, priority: u8) {
        self.items.push(PrioListItem::new(tok, priority));
    }

    pub fn remove(&mut self, tok: mio::Token) {
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
                self.current = Some(index);
            }
        } else {
            self.current = Some(index);
        }
    }

    fn find_activable_index(&self, tok: mio::Token) -> Option<usize> {
        self.items.
            iter().
            enumerate().
            filter(|&(_, item)| item.token == tok && item.active == false).
            nth(0).
            map(|(i, _)| i)
    }

    pub fn advance(&mut self) {
        if let Some(index) = self.current {
            if let Some(i) = self.find_active_index_after(index + 1) {
                self.current = Some(i);
            }
        }
    }

    fn find_active_index_after(&self, from: usize) -> Option<usize> {
        if from == self.items.len() {
            None
        } else {
            for i in from..self.items.len() {
                if self.items[i].active {
                    return Some(i);
                }
            }

            None
        }
    }

    pub fn deactivate_and_advance(&mut self) {
        
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
    fn advance_whith_single_active_item_loops() {
        let mut list = PrioList::new();

        list.insert(mio::Token(10), 8);
        list.activate(mio::Token(10));
        list.advance();
        assert_eq!(Some(mio::Token(10)), list.get());
    }

    #[test]
    fn advance_can_move_forward() {
        let mut list = PrioList::new();
        
        list.insert(mio::Token(10), 8);
        list.insert(mio::Token(20), 8);

        list.activate(mio::Token(10));
        list.activate(mio::Token(20));
        list.activate(mio::Token(30));
        list.advance();
        assert_eq!(Some(mio::Token(20)), list.get());
    }
}
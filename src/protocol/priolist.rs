// Copyright 2016 Benoît Labaere (benoit.labaere@gmail.com)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0>
// or the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to those terms.



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

// let's ignore priority for now.

use mio;

pub struct PrioList {
    pending: Vec<mio::Token>,
    tokens: Vec<mio::Token>,
    current: Option<mio::Token>
}

impl PrioList {
    pub fn new() -> PrioList {
        PrioList {
            pending: Vec::new(),
            tokens: Vec::new(),
            current: None
        }
    }

    pub fn insert(&mut self, token: mio::Token, _: usize) {
        // TODO blabaere 24/11/2015 deal with priorities ...
        if let Err(index) = self.pending.binary_search(&token) {
            self.pending.insert(index, token);
        }
    }

    pub fn remove(&mut self, token: &mio::Token) {
        if let Ok(index) = self.pending.binary_search(&token) {
            self.pending.remove(index);
        }
        else if let Ok(index) = self.tokens.binary_search(&token) {
            let token = self.tokens.remove(index);

            if Some(token) == self.current {
                self.current = if self.tokens.is_empty() {
                    None
                } else {
                    Some(self.tokens[0])
                }
            }
        }
    }

    pub fn get(&self) -> Option<mio::Token> {
        self.current
    }

    pub fn activate(&mut self, token: mio::Token) {
        let mut activated = false;
        if let Ok(index) = self.pending.binary_search(&token) {
            let token = self.pending.remove(index);

            if let Err(index) = self.tokens.binary_search(&token) {
                self.tokens.insert(index, token);
                activated = true;
            }
        } else if let Ok(_) = self.tokens.binary_search(&token) {
            activated = true;
        }
        if activated && self.current.is_none() {
            self.current = Some(token);
        }
    }

    pub fn advance(&mut self) {
        if let Some(token) = self.current {
            if let Ok(index) = self.tokens.binary_search(&token) {
                let next = (index + 1) % self.tokens.len();

                self.current = Some(self.tokens[next]);
            }
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
    fn when_first_item_is_added_it_is_inactive() {
        let mut list = PrioList::new();
        let item = mio::Token(1);

        list.insert(item, 8);

        assert_eq!(None, list.get())
    }

    #[test]
    fn when_first_item_is_activated_it_becomes_current() {
        let mut list = PrioList::new();
        let item = mio::Token(1);

        list.insert(item, 8);
        list.activate(item);

        assert_eq!(Some(item), list.get())
    }

    #[test]
    fn when_second_item_is_activated_it_does_not_become_current() {
        let mut list = PrioList::new();
        let item1 = mio::Token(3);
        let item2 = mio::Token(1);

        list.insert(item1, 8);
        list.activate(item1);

        list.insert(item2, 8);
        list.activate(item2);

        assert_eq!(Some(item1), list.get())
    }

    #[test]
    fn when_one_active_token_advance_does_nothing() {
        let mut list = PrioList::new();
        let item = mio::Token(1);

        list.insert(item, 8);
        list.activate(item);
        list.advance();

        assert_eq!(Some(item), list.get())
    }

    #[test]
    fn when_several_tokens_advance_from_head_moves_next() {
        let mut list = PrioList::new();
        let item1 = mio::Token(1);
        let item2 = mio::Token(2);
        let item3 = mio::Token(3);

        list.insert(item1, 8);
        list.activate(item1);
        assert_eq!(Some(item1), list.get());

        list.insert(item2, 8);
        list.activate(item2);
        assert_eq!(Some(item1), list.get());
        
        list.insert(item3, 8);
        list.activate(item3);
        assert_eq!(Some(item1), list.get());
        
        list.advance();
        assert_eq!(Some(item2), list.get());
        
        list.advance();
        assert_eq!(Some(item3), list.get());
        
        list.advance();
        assert_eq!(Some(item1), list.get());
    }

    #[test]
    fn remove_from_empty_list_does_nothing() {
        let mut list = PrioList::new();
        let item1 = mio::Token(1);

        list.remove(&item1);

        assert_eq!(None, list.get())
    }

    #[test]
    fn remove_inactive_item_does_nothing() {
        let mut list = PrioList::new();
        let item1 = mio::Token(1);

        list.insert(item1, 8);
        list.remove(&item1);
        
        assert_eq!(None, list.get())
    }

    #[test]
    fn remove_last_active_item_clear_current() {
        let mut list = PrioList::new();
        let item1 = mio::Token(1);
        let item2 = mio::Token(2);

        list.insert(item1, 8);
        list.insert(item2, 8);
        list.activate(item1);
        list.remove(&item1);
        
        assert_eq!(None, list.get())
    }

    #[test]
    fn remove_active_item_another_become_current() {
        let mut list = PrioList::new();
        let item1 = mio::Token(1);
        let item2 = mio::Token(2);

        list.insert(item1, 8);
        list.insert(item2, 8);
        list.activate(item1);
        list.activate(item2);
        list.remove(&item2);
        
        assert_eq!(Some(item1), list.get())
    }

    #[test]
    fn remove_inactive_item_do_not_change_current() {
        let mut list = PrioList::new();
        let item1 = mio::Token(1);
        let item2 = mio::Token(2);

        list.insert(item1, 8);
        list.insert(item2, 8);
        list.activate(item1);
        list.activate(item2);
        list.remove(&item1);
        
        assert_eq!(Some(item2), list.get())
    }

    #[test]
    fn activate_not_inserted_item_does_nothing() {
        let mut list = PrioList::new();
        let item1 = mio::Token(1);

        list.activate(item1);
        
        assert_eq!(None, list.get())
    }
}
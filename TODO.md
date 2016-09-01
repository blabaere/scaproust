### REQ resend 
Resend must be scheduled when a regular send succeeds, and cancelled when the matching recv occurs.
What if a user command is received is when a resend is in progress (some bytes sent, but not all) ?

BONUS: if the pipe that the request was sent to is removed, the request could be resent right away ...

### Next tasks
- LINGER !!!
- Handle accept error

### AUTOMATE ALL THE THINGS (test) !!!
Test reconnect feature

### Features:
- Implement nanocat
- STAR protocol ?

### Performance
- Use a pool for payloads and buffers (if any)

### change copyright header to mention 'authors' and the AUTHORS file

### Stuff to look at:

https://github.com/frankmcsherry/recycler
https://github.com/zslayton/lifeguard
http://carllerche.github.io/pool/pool/

**mioco now has a timeout feature !**  
https://github.com/dpc/mioco  
https://github.com/dpc/mioco/blob/master/examples%2Fechoplus.rs  

https://github.com/tailhook/rotor  
https://github.com/dwrensha/gj  
https://github.com/zonyitoo/simplesched  
https://github.com/alexcrichton/wio (for appveyor ci script and doc publication too)  
https://github.com/burrows-labs/mio-websockets  

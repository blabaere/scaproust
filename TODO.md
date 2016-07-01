### 'readable' + 'hup' inside a loop with handshake and message  in the buffer
This goes wrong, probably because the resubscription fails due to hup.

### use macros for factorizing common protocol code ?

### define a struct wrapping the event loop reference and the notification token
There is too much coupling between the protocol and the event loop.
Stuff like registration and timeout should be exposed by that struct.

### REQ resend 
Resend must be scheduled when a regular send succeeds, and cancelled when the matching recv occurs.
What if a user command is received is when a resend is in progress (some bytes sent, but not all) ?

BONUS: if the pipe that the request was sent to is removed, the request could be resent right away ...

### Receving a malformed message will cause the recv operation to end in timeout
When a protocol receives a "malformed" message, the message is dropped, but the facade is not notified of anything and no pipe is asked to recv again

### Next tasks
- LINGER !!!
- Handle accept error

### AUTOMATE ALL THE THINGS !!!

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

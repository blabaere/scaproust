### Make broadcast `send_blocked` aware
When a pipe raises a `send_blocked`, it should be removed from the list, and added back when `send_done`.

### When the sending/receiving pipe is removed, it probably means the operation has failed

### REQ resend cannot be implemented with the current design
There can be only one operation in progress for a given socket but resend occurs in background.
Resend must be scheduled when a regular send succeeds, and cancelled when the matching recv occurs.
What if a user command is received is when a resend is in progress (some bytes sent, but not all).

BONUS: if the pipe that the request was sent to is removed, the request could be resent right away ...

### Receving a malformed message will cause the recv operation to end in timeout
When a protocol receives a "malformed" message, the message is dropped, but the facade is not notified of anything and no pipe is asked to recv again

### Next problems
- Create a core folder, for socket, acceptor and session modules
- Create a facade folder for modules named *_facade
- Create an io folder (or something better ?) for pipe, send and recv modules
- Add some doc comment on each SocketType variant
- Have pipe error forwarded to the session and the socket
- Handle accept error

### Refactors:
- Rename session into something less oriented ? (context, environment ...)
- Implement Read & Write trait on sockets
- Use a pool for payloads and buffers (if any)

### Features:
- Implement nanocat
- Expose the event loop configuration ?

### Stuff to look at:
**mioco now has a timeout feature !**  
https://github.com/dpc/mioco  
https://github.com/dpc/mioco/blob/master/examples%2Fechoplus.rs  

https://github.com/tailhook/rotor  
https://github.com/dwrensha/gj  
https://github.com/zonyitoo/simplesched  
https://github.com/alexcrichton/wio (for appveyor ci script and doc publication too)  
https://github.com/burrows-labs/mio-websockets  

### Windows problem: non-blocking send not available
The way mio works on Windows currently makes it impossible to send several chunks of bytes
without having to wait for an event loop round-trip.
This is required for dist based protocols to work (pub, survey and bus).
The applied workaround is to create a buffer for each non-blocking send operation.

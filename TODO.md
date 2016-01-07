### Current problem: REQ resend
There can be only one operation in progress for a given socket but resend occurs in background.
Resend must be scheduled when a regular send succeeds, and cancelled when the matching recv occurs.
What if a user command is received is when a resend is in progress (a some bytes sent, but not all).

BONUS: if the pipe that the request was sent to is removed, the request could be resent right away ...

### Next problems
- Handle accept error
- Have pipe error forwarded to the session and the socket
- Session shutdown should unblock current socket operations

### General:
- write documentation
- adds embedded code example to the front page
- setup documentation generation and site and github pages

### Refactors:
- Rename session into something less oriented ? (context, environment ...)
- Implement Read & Write trait on sockets
- Use a pool for payloads

### Features:
- For raw bus socket, store the pipe token in the header when receving a message. When sending, check if there is a pipe token in the header and skip the specified pipe
- Have Socket::connect & bind return an Endpoint that can be shut down
- Implement nanocat
- Expose the event loop configuration ?

### Stuff to look at:
https://github.com/tailhook/rotor  
https://github.com/dpc/mioco  
https://github.com/dwrensha/gj  
https://github.com/zonyitoo/simplesched  
https://github.com/alexcrichton/wio (for appveyor ci script and doc publication too)  
https://github.com/burrows-labs/mio-websockets  

### Windows problem: non-blocking send not available
The way mio works on Windows currently makes it impossible to send several chunks of bytes
without having to wait for an event loop round-trip.
This is required for dist based protocols to work (pub, survey and bus).
The applied workaround is to create a buffer for each non-blocking send operation.

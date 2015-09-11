Update rust-closure-playground repo with FnBox findings


### General:
- write documentation
- adds embedded code example to the front page
- setup documentation generation and site and github pages
- setup CI with appveyor once mio is compatible with windows

### Refactors:  
- Rename session into something less oriented ? (context, environment ...)
- Remove association between token and socketid when a pipe is dead
- Register to readable or writable only when required, that is when operation status is in progress ? 
- maybe the acceptor could create pipes instead of connections ?
- Implement Read & Write trait on sockets
- Use a pool for payloads

### Features:
- For raw bus socket, store the pipe token in the header when receving a message. When sending, check if there is a pipe token in the header and skip the specified pipe
- Check what to do when send/recv timeout is reached and parts of the message has already been transfered !
- Have Socket::connect & bind return an Endpoint that can be shut down
- Implement nanocat
- Expose the event loop configuration ?

### Stuff to look at:
https://github.com/dpc/mioco  
https://github.com/dwrensha/gj  
https://github.com/zonyitoo/simplesched  
https://github.com/alexcrichton/wio (for appveyor ci script and doc publication too)  
https://github.com/burrows-labs/mio-websockets  
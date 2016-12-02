### Modules

- Facade : the public API of scaproust, the components in this module are just a wrapper over a bidirectional channel connected to the backend. Each method call is translated into a write and then a read in the channel.  

- Core : implements or specify the core concepts of scaproust. In short these are session, socket, endpoint and protocol. It contains the backend counterpart of each facade component.

- Reactor : interaction with the i/o polling, event loop and event dispatcher, receives the requests sent by the facade, routing between them core and transport components.

- Protocol : implementation of each scalability protocol

- Transport : implementation of each transport media. Responsible for providing `connect` and `bind` feature, thus creating physical endpoints and then transfering messages over the endpoints.

### Naming convention for enums used to communicate over event loop or channel:  

- Request : sent by the facade to the backend when the user API is called.
- Reply: sent by the backend to the facade in response to a request.  
Request/Reply are not implemented as regular method call because facade and backend components are not controlled by the same thread.  
- Cmd: command sent to backend components, it is also the input of state machines.
- Evt: event published by backend components, as reaction to command, event or i/o readiness callback.  
Cmd/Evt are not implemented as regular method call because it would require circular references.

### Threading model:  

One thread to rule them all: creating a session will start a thread where all I/O operations will occurs. This is made possible by using [mio](https://github.com/carllerche/mio) which exposes only non-blocking I/O primitives and a polling system. This I/O thread loops over a poll function and fowards the various readiness changes to each impacted backend component. The channel that allows the communication with the user threads is itself plugged to the polling system so the loop wakes up when required.  

Example of workflow (simplified):  
  
1. The user thread call `socket::send`.
2. The facade socket sends a request through its channel.
3. The facade socket starts a blocking read on its channel.
4. The user thread is now blocked, waiting for a reply on the socket channel.
5. The I/O thread call to `poll` returns a collection of events, one of them is the channel readiness.
6. The dispatcher forwards the request to the backend socket.
7. The backend socket selects a ready endpoint.
8. The endpoint writes the message content to the media (TCP, IPC ...).
9. If the send is complete, the socket sends a reply through its channel.  
10. The user thread call to `channel::recv` returns a success code.
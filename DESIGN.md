Modules

- Facade : the public API of scaproust, the components in this module are just a wrapper over a bidirectional channel connected to the backend. Each method call is translated into a write and then a read in the channel.  

- Core : implements or specify the core concept of scaproust, session, socket, endpoint and protocol. Contains the backend counterpart of each facade component.

- Ctrl : interaction with the event loop, receives the requests sent by the facade, routing between core and transport components.

- Protocol : implementation of each scalability protocol

- Transport : implementation of each transport media. Responsible for providing connect and bind feature, thus creating physical endpoint and then transfering message over the endpoints.

Naming convention for enums used to communicate over event loop or channel:  
Request: sent by the facade to the backend when the user API is called
Reply: sent by the backend to the facade in response to a request.
Request/Reply are not implemented as regular method call because facade and backend components are not controlled by the same thread.
Cmd: command sent to backend components, it is also the input of state machines.
Evt: event published by backend components, as reaction to command, event or i/o readiness callback.
Cmd/Evt are not implemented as regular method call because it would require circular references.

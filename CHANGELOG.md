## 0.3.2 (2018-01-12)

### Changed
- Updates dependencies.

### Fixed
- Sending an empty message should not crash the session.

## 0.3.1 (2017-08-21)

### Changed
- Updates rustc requirement to stable instead of nightly.
- Small perf improvement (thanks to iovec).

## 0.3.0 (2017-03-04)

### Added
- IPC transport on Windows, using named pipes.

### Fixed
- Fix perf issue with TCP transport on *nix
- Remove hard dependency on clippy

## 0.2.0 (2016-11-20)

### Added
- Non-blocking versions of send and recv
- Socket polling via a dedicated probe component.
- Make the transports pluggable.
- Let the user close individual endpoints.
- IPC transport on *nix, using unix socket.
- Expose `recv max size` and `tcp no delay` options. 

### Changed
- A builder must be used to create a session.

### Fixed
- Fix potential infinite wait in device
- Fix many-to-many topology with REQ/REP and SURV/RESP over device

## 0.1.0 (2016-09-02)

Initial release

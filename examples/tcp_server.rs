//! The example below shows a simple non-blocking TCP server.
//!
//! It will write the connection's ip address and close the connection.

use std::collections::HashMap;
use std::io::{self, Write};
use std::net::SocketAddr;

use mio_st::net::{TcpListener, TcpStream};
use mio_st::os::{OsQueue, RegisterOption};
use mio_st::{event, poll};

// An unique id to associate an event with a handle, in this case for our TCP
// listener.
const SERVER_ID: event::Id = event::Id(0);

fn main() -> Result<(), Box<std::error::Error>> {
    // Create a Operating System backed (epoll or kqueue) queue.
    let mut os_queue = OsQueue::new()?;
    // Create our event sink.
    let mut events = Vec::new();

    // Setup a TCP listener.
    let address = "127.0.0.1:12345".parse()?;
    let mut server = TcpListener::bind(address)?;

    // Register our TCP listener with `OsQueue`, this allows us to receive
    // readiness events about incoming connections.
    os_queue.register(&mut server, SERVER_ID, TcpListener::INTERESTS,
        RegisterOption::EDGE)?;

    // A hashmap with `event::Id` -> `(TcpStream, SocketAddr)` connections.
    let mut connections = HashMap::new();

    // A simple counter to create new unique ids for each incoming connection.
    let mut current_id = event::Id(10);

    println!("Listening on {}", address);
    println!("Run `nc {} {}` to test it", address.ip(), address.port());

    // Our event loop.
    loop {
        // Poll for events.
        poll::<_, io::Error>(&mut [&mut os_queue], &mut events, None)?;

        // Process each event.
        for event in events.drain(..) {
            // Depending on the event id we need to take an action.
            match event.id() {
                SERVER_ID => {
                    // The server is ready to accept one or more connections.
                    accept_connections(&mut server, &mut os_queue, &mut connections, &mut current_id)?;
                },
                event_id => {
                    // A connection is possibly ready, but it might a spurious
                    // event.
                    let done = match connections.get_mut(&event_id) {
                        Some((stream, peer_address)) => {
                            // Write the peer address to the connection, returns
                            // true if we're done with the connection.
                            write_address(event_id, stream, *peer_address)
                        },
                        // Spurious event, we can safely ignore it.
                        None => continue,
                    };

                    // If we're done with the connection remove it from the map.
                    if done {
                        let connection = connections.remove(&event_id);
                        assert!(connection.is_some());
                    }
                },
            }
        }
    }
}

/// Accept connection from the TCP `listener`, register the connection with the
/// `os_queue` with an unique id and store the connection in the `connections`
/// map.
fn accept_connections(
    listener: &mut TcpListener, os_queue: &mut OsQueue,
    connections: &mut HashMap<event::Id, (TcpStream, SocketAddr)>,
    current_id: &mut event::Id,
) -> io::Result<()> {
    // Since we registered with edge-triggered events for our server we need to
    // accept connections until we hit a would block "error".
    loop {
        let (mut connection, address) = match listener.accept() {
            Ok((connection, address)) => (connection, address),
            Err(ref err) if would_block(err) => return Ok(()),
            Err(err) => return Err(err),
        };

        // Generate a new id for the connection.
        let id = new_id(current_id);
        println!("Accepted a new connection from: {}: id={}", address, id);

        // Register the TCP connection so we can handle events for it as well.
        os_queue.register(&mut connection, id, TcpStream::INTERESTS, RegisterOption::EDGE)?;

        // Store our connection so we can access it later.
        connections.insert(id, (connection, address));
    }
}

/// Generate a new id (the next one) based on the `current` id.
fn new_id(current: &mut event::Id) -> event::Id {
    let id = *current;
    *current = event::Id(current.0 + 1);
    id
}

/// Write `peer_addres` to the `stream`. Returns true if connection should be
/// removed from the connections map.
fn write_address(id: event::Id, stream: &mut TcpStream, peer_address: SocketAddr) -> bool {
    let peer_address = peer_address.to_string();
    let err = match stream.write(peer_address.as_bytes()) {
        // If the entire address was written then we're done.
        Ok(bytes_written) if bytes_written == peer_address.len() => return true,
        // If not the entire address was written then we convert it into a short
        // written error.
        Ok(_) => io::ErrorKind::WriteZero.into(),
        // In case of a would block "error" we return false and try again later.
        Err(ref err) if would_block(err) => return false,
        // Other errors we return.
        Err(err) => err,
    };

    eprintln!("Error writing to connection: {}: id={}", err, id);
    // The connection is broken we can remove it from the map, the client will
    // have to reconnect.
    return true;
}

/// Returns true if `err` is of kind `WouldBlock`, one we can safely ignore.
fn would_block(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::WouldBlock
}

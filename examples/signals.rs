use std::io;

use mio_st::os::{Signal, SignalSet, Signals};
use mio_st::{event, poll, OsQueue};

const SIGNAL_ID: event::Id = event::Id(10);

fn main() -> io::Result<()> {
    // Create our event sink and an OS queue.
    let mut events = Vec::new();
    let mut os_queue = OsQueue::new()?;

    // Create the signaler and register all possible signals.
    let mut signals = Signals::new(&mut os_queue, SignalSet::all(), SIGNAL_ID)?;

    loop {
        // Poll for events.
        poll::<_, io::Error>(&mut [&mut os_queue], &mut events, None)?;

        // Now send the process a signal, e.g. by pressing `CTL+C` in a shell,
        // or calling `kill` on it.

        // Process each event.
        for event in events.drain(..) {
            match event.id() {
                SIGNAL_ID => {
                    // Receive the signal send.
                    match signals.receive()? {
                        Some(Signal::Interrupt) => println!("Got interrupt signal"),
                        Some(Signal::Terminate) => {
                            println!("Got terminate signal");
                            return Ok(());
                        },
                        Some(Signal::Quit) => println!("Got quit signal"),
                        Some(Signal::Continue) => println!("Got continue signal"),
                        _ => println!("Got unknown signal event: {:?}", event),
                    }
                },
                _ => println!("Got unknown event: {:?}", event),
            }
        }
    }
}

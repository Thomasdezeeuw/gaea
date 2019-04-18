use std::io::{self, Read};
use std::ops::{Deref, DerefMut};
use std::panic;
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

use mio_st::event;
use mio_st::os::{Signal, Signals, SignalSet};

mod util;

use self::util::init_with_os_queue;

#[test]
fn signal_bit_or() {
    // `Signal` and `Signal` (and `Signal`).
    assert_eq!(Signal::Terminate | Signal::Quit | Signal::Interrupt, SignalSet::all());
    // `Signal` and `SignalSet`.
    assert_eq!(Signal::Terminate | SignalSet::empty(), Signal::Terminate.into());

    // `SignalSet` and `Signal`.
    assert_eq!(SignalSet::empty() | Signal::Quit, Signal::Quit.into());
    // `SignalSet` and `SignalSet`.
    assert_eq!(SignalSet::empty() | Signal::Interrupt, Signal::Interrupt.into());

    // Overwriting.
    assert_eq!(Signal::Terminate | Signal::Terminate, Signal::Terminate.into());
    assert_eq!(Signal::Terminate | SignalSet::all(), SignalSet::all());
    assert_eq!(SignalSet::all() | Signal::Quit, SignalSet::all());
    assert_eq!(SignalSet::all() | SignalSet::all(), SignalSet::all());
    assert_eq!(SignalSet::all() | SignalSet::empty(), SignalSet::all());
}

#[test]
fn signal_set() {
    let tests = vec![
        (SignalSet::empty(), 0, vec![]),
        (SignalSet::all(), 3, vec![Signal::Interrupt, Signal::Terminate, Signal::Quit]),
        (Signal::Interrupt.into(), 1, vec![Signal::Interrupt]),
        (Signal::Terminate.into(), 1, vec![Signal::Terminate]),
        (Signal::Quit.into(), 1, vec![Signal::Quit]),
        (Signal::Interrupt | Signal::Terminate, 2, vec![Signal::Interrupt, Signal::Terminate]),
        (Signal::Interrupt | Signal::Quit, 2, vec![Signal::Interrupt, Signal::Quit]),
        (Signal::Terminate | Signal::Quit, 2, vec![Signal::Terminate, Signal::Quit]),
        (Signal::Interrupt | Signal::Terminate | Signal::Quit, 3,
            vec![Signal::Interrupt, Signal::Terminate, Signal::Quit]),
    ];

    for (set, size, expected) in tests {
        let set: SignalSet = set;
        assert_eq!(set.size(), size);

        // Test `contains`.
        let mut contains_iter = (&expected).into_iter().cloned();
        while let Some(signal) = contains_iter.next() {
            assert!(set.contains(signal));
            assert!(set.contains::<SignalSet>(signal.into()));

            // Set of the remaining signals.
            let mut contains_set: SignalSet = signal.into();
            for signal in contains_iter.clone() {
                contains_set = contains_set | signal;
            }
            assert!(set.contains(contains_set));
        }

        // Test `SignalSetIter`.
        assert_eq!(set.into_iter().len(), size);
        assert_eq!(set.into_iter().count(), size);
        assert_eq!(set.into_iter().size_hint(), (size, Some(size)));
        let signals: Vec<Signal> = set.into_iter().collect();
        assert_eq!(signals.len(), expected.len());
        for expected in expected {
            assert!(signals.contains(&expected));
        }
    }
}

#[test]
fn receive_no_signal() {
    let (mut os_queue, _) = init_with_os_queue();

    let id = event::Id(0);
    let mut signals = Signals::new(&mut os_queue, SignalSet::all(), id)
        .expect("unable to create Signals");
    assert_eq!(signals.receive().expect("unable to receive signal"), None);
}

#[test]
fn signals_example() {
    let child = run_example("signals");

    // Give the process some time to startup.
    sleep(Duration::from_millis(200));

    let pid = child.id() as libc::pid_t;

    send_signal(pid, libc::SIGINT);
    send_signal(pid, libc::SIGQUIT);
    send_signal(pid, libc::SIGTERM);

    let output = read_output(child);
    assert_eq!(output, "Got interrupt signal\nGot quit signal\nGot terminate signal\n");
}

/// Wrapper around a `command::Child` that kills the process when dropped, even
/// if the test failed. Sometimes the child command would survive the test when
/// running then in a loop (e.g. with `cargo watch`). This caused problems when
/// trying to bind to the same port again.
struct ChildCommand {
    inner: Child,
}

impl Deref for ChildCommand {
    type Target = Child;

    fn deref(&self) -> &Child {
        &self.inner
    }
}

impl DerefMut for ChildCommand {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.inner
    }
}

impl Drop for ChildCommand {
    fn drop(&mut self) {
        let _ = self.inner.kill();
        self.inner.wait().expect("can't wait on child process");
    }
}

/// Run an example, not waiting for it to complete, but it does wait for it to
/// be build.
fn run_example(name: &'static str) -> ChildCommand {
    build_example(name);
    start_example(name)
}

/// Build the example with the given name.
fn build_example(name: &'static str) {
    let output = Command::new("cargo")
        .args(&["build", "--example", name])
        .output()
        .expect("unable to build example");

    if !output.status.success() {
        panic!("failed to build example: {}\n\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr));
    }
}

/// Start and already build example
fn start_example(name: &'static str) -> ChildCommand {
    Command::new(format!("target/debug/examples/{}", name))
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map(|inner| ChildCommand { inner })
        .expect("unable to run example")
}

fn send_signal(pid: libc::pid_t, signal: libc::c_int) {
    if unsafe { libc::kill(pid, signal) } == -1 {
        let err = io::Error::last_os_error();
        panic!("error sending signal: {}", err);
    }
}

/// Read the standard output of the child command.
fn read_output(mut child: ChildCommand) -> String {
    child.wait().expect("error running example");

    let mut stdout = child.stdout.take().unwrap();
    let mut output = String::new();
    stdout.read_to_string(&mut output).expect("error reading output of example");
    output
}

#[test]
#[should_panic(expected = "can't create `Signals` with an empty signal set")]
fn sender_readable_interests() {
    let (mut os_queue, _) = init_with_os_queue();

    let _signals = Signals::new(&mut os_queue, SignalSet::empty(), event::Id(0))
        .unwrap();
}

use std::io::{self, Read};
use std::ops::{Deref, DerefMut};
use std::panic;
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

#[test]
fn signals_examples() {
    let child = run_example("signals");

    // Give the process some time to startup.
    sleep(Duration::from_millis(200));

    let pid = child.id() as libc::pid_t;

    send_signal(pid, libc::SIGINT);
    send_signal(pid, libc::SIGSTOP);
    send_signal(pid, libc::SIGCONT);
    send_signal(pid, libc::SIGQUIT);
    send_signal(pid, libc::SIGTERM);

    let output = read_output(child);
    assert_eq!(output, "Got interrupt signal\nGot continue signal\nGot quit signal\nGot terminate signal\n");
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

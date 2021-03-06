use b7::b7tui::Env;
use b7::dynamorio;
use b7::B7Opts;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use ctor::ctor;

// This hack ensures that we block SIGCHLD
// on every thread. When running tests,
// Rust spawns several test worker threads
// from the main thread. In order to
// ensure that *every* thread (including the main thread)
// has SIGCHLD blocked, we use the 'ctor' crate to run
// our code very early during process startup.
//
// This is not a normal function - main() has not
// yet been called, any the Rust stdlib may not yet
// be initialized. It should do the absolute minimum
// necessary to get B7 working in a test environment
#[ctor]
fn on_init() {
    b7::process::block_signal();
}

#[test]
fn run_wyv() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut dynpath = path.clone();

    path.push("tests");
    path.push("wyvern");

    dynpath.push("dynamorio");
    dynpath.push("build");

    let mut term = Env::new();
    let mut vars = HashMap::new();
    vars.insert(
        "dynpath".to_string(),
        dynpath.to_string_lossy().into_owned(),
    );

    let mut opts = B7Opts::new(
        path.to_string_lossy().into_owned(),
        false,
        true,
        Box::new(dynamorio::DynamorioSolver),
        &mut term,
        vars,
        Duration::new(5, 0),
    );

    let res = opts.run();
    let mut stdin = res.stdin_brute;

    // Last character is currently non-deterministic
    stdin.pop();
    assert_eq!(&stdin, "dr4g0n_or_p4tric1an_it5_LLVM");
}

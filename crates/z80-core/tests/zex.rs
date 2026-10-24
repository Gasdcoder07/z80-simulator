//! The ZEXDOC and ZEXALL conformance suites.
//!
//! Each suite runs several thousand instruction sequences and compares a CRC
//! of the resulting register and flag state against values captured from real
//! hardware. ZEXDOC checks documented flag behaviour only; ZEXALL also checks
//! the undocumented flags 3 and 5.
//!
//! They take tens of billions of cycles, so both are `#[ignore]`d by default:
//!
//! ```text
//! cargo test -p z80-core --release -- --ignored --nocapture
//! ```

mod common;

use common::run_cpm;

fn run_suite(name: &str, program: &[u8]) {
    let mut failures = Vec::new();

    let (output, tstates) = run_cpm(program, |line| {
        if !line.is_empty() {
            println!("{line}");
        }
        if line.contains("ERROR") {
            failures.push(line.to_string());
        }
    });

    println!("{name}: {tstates} T-states simulated");

    assert!(
        failures.is_empty(),
        "{name} reported {} failing test(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(
        output.contains("Tests complete"),
        "{name} did not run to completion; output was:\n{output}"
    );
}

#[test]
#[ignore = "runs tens of billions of cycles; use --release"]
fn zexdoc() {
    run_suite(
        "ZEXDOC",
        include_bytes!("../../../tests/conformance/bin/zexdoc.com"),
    );
}

#[test]
#[ignore = "runs tens of billions of cycles; use --release"]
fn zexall() {
    run_suite(
        "ZEXALL",
        include_bytes!("../../../tests/conformance/bin/zexall.com"),
    );
}

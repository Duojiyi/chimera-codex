// Step 9.1 RED — injectable clock.
//
// Verification logic must never read the system clock directly, or the
// freeze-attack test (metadata that is valid except its expiry has passed)
// could not exist without sleeping in a test suite.

use chimera_update::clock::{Clock, FixedClock};

#[test]
fn a_fixed_clock_reports_exactly_the_value_it_was_given() {
    let clock = FixedClock(1_700_000_000);
    assert_eq!(clock.now(), 1_700_000_000);
}

#[test]
fn two_fixed_clocks_can_disagree_so_tests_can_simulate_time_passing() {
    let before = FixedClock(100);
    let after = FixedClock(200);
    assert!(after.now() > before.now());
}

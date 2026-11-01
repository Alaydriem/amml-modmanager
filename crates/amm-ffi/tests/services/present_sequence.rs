use amm_ffi::services::PresentSequence;

#[test]
fn each_present_gets_the_next_number_and_current_reads_the_newest() {
    // the count is process-wide and other tests may advance it, so only the order is checked
    let first = PresentSequence::next();
    let second = PresentSequence::next();

    assert!(first >= 1);
    assert!(second > first);
    assert!(PresentSequence::current() >= second);
}

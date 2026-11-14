use super::*;

#[test]
fn verifies_sync_backoff_calculations() {
    let backoff = SyncBackoff::new(5, 60, 2.0);

    assert_eq!(backoff.compute_delay_secs(0), 0);
    assert_eq!(backoff.compute_delay_secs(1), 5); // 5 * 2^0 = 5
    assert_eq!(backoff.compute_delay_secs(2), 10); // 5 * 2^1 = 10
    assert_eq!(backoff.compute_delay_secs(3), 20); // 5 * 2^2 = 20
    assert_eq!(backoff.compute_delay_secs(4), 40); // 5 * 2^3 = 40
    assert_eq!(backoff.compute_delay_secs(5), 60); // capped at 60
    assert_eq!(backoff.compute_delay_secs(10), 60); // capped at 60
}

#[test]
fn verifies_sync_scheduler_triggers_and_debouncing() {
    // Debounce: 3 seconds, Periodic: 300 seconds
    let mut scheduler = SyncScheduler::new(3, 300);

    // 1. Initial startup sync
    assert_eq!(scheduler.check_trigger(1000), Some(SyncTrigger::Startup));

    // Record successful startup sync at t=1000
    scheduler.record_success(1000);
    assert_eq!(scheduler.check_trigger(1001), None);

    // 2. Notify local mutation at t=1010
    scheduler.notify_local_mutation(1010);
    // Before debounce deadline (deadline is 1010 + 3 = 1013)
    assert_eq!(scheduler.check_trigger(1011), None);
    assert_eq!(scheduler.check_trigger(1012), None);
    // At deadline -> triggers LocalMutation
    assert_eq!(
        scheduler.check_trigger(1013),
        Some(SyncTrigger::LocalMutation)
    );

    // Record failure at t=1013
    scheduler.record_failure(1013);
    assert_eq!(scheduler.consecutive_failures, 1);

    // Backoff delay for 1 failure is 5s, so next retry not allowed until 1013 + 5 = 1018
    assert_eq!(scheduler.check_trigger(1015), None);
    assert_eq!(scheduler.check_trigger(1017), None);
    assert_eq!(
        scheduler.check_trigger(1018),
        Some(SyncTrigger::LocalMutation)
    );

    // Record success at t=1018
    scheduler.record_success(1018);
    assert_eq!(scheduler.consecutive_failures, 0);
    assert_eq!(scheduler.check_trigger(1019), None);

    // 3. Periodic trigger after 300s (at t=1018 + 300 = 1318)
    assert_eq!(scheduler.check_trigger(1317), None);
    assert_eq!(scheduler.check_trigger(1318), Some(SyncTrigger::Periodic));
}

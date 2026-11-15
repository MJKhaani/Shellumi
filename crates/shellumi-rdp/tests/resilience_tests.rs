//! Network resilience and automatic reconnection integration tests for Shellumi RDP client.

use shellumi_rdp::client::run_rdp_session;
use shellumi_remote_desktop::{
    create_session_channels, Framebuffer, ReconnectPolicy, RemoteProtocol, SessionCommand,
    SessionConfig, SessionEvent, SessionState,
};
use std::time::Duration;
use tokio::net::TcpListener;

#[test]
fn test_rdp_reconnect_policy_default_and_custom_backoff() {
    let policy = ReconnectPolicy::default();
    assert!(policy.enabled);
    assert_eq!(policy.max_attempts, 5);
    assert_eq!(policy.initial_delay_ms, 1000);
    assert_eq!(policy.max_delay_ms, 15000);
    assert_eq!(policy.backoff_multiplier, 2);

    assert_eq!(policy.compute_delay(1), Duration::from_millis(1000));
    assert_eq!(policy.compute_delay(2), Duration::from_millis(2000));
    assert_eq!(policy.compute_delay(3), Duration::from_millis(4000));
    assert_eq!(policy.compute_delay(4), Duration::from_millis(8000));
    assert_eq!(policy.compute_delay(5), Duration::from_millis(15000)); // clamped to max_delay_ms

    let disabled = ReconnectPolicy::disabled();
    assert!(!disabled.enabled);
    assert_eq!(disabled.max_attempts, 0);
}

#[tokio::test]
async fn test_rdp_session_resilience_unreachable_endpoint_exhaustion() {
    // Bind and drop an ephemeral TCP port so connection attempts fail fast
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("Skipping live endpoint test: network sandboxed ({e})");
            return;
        }
        Err(e) => panic!("Failed to bind loopback: {e}"),
    };
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let (ui_handle, worker_handle) = create_session_channels(32, 64);
    let mut config = SessionConfig::new("127.0.0.1", port, RemoteProtocol::Rdp);
    config.reconnect_policy = ReconnectPolicy {
        enabled: true,
        max_attempts: 2,
        initial_delay_ms: 10,
        max_delay_ms: 30,
        backoff_multiplier: 1,
    };

    let fb = Framebuffer::new(800, 600).unwrap();
    let runner_task = tokio::spawn(async move { run_rdp_session(config, worker_handle, fb).await });

    let mut attempts = Vec::new();
    let mut saw_disconnected = false;

    while let Ok(event) = ui_handle.recv_event().await {
        match event {
            SessionEvent::StateChanged(SessionState::Reconnecting { attempt, .. }) => {
                attempts.push(attempt);
            }
            SessionEvent::StateChanged(SessionState::Disconnected) => {
                saw_disconnected = true;
                break;
            }
            _ => {}
        }
    }

    assert_eq!(
        attempts,
        vec![1, 2],
        "Expected 2 reconnect attempts before exhaustion"
    );
    assert!(
        saw_disconnected,
        "Expected Disconnected state after retry exhaustion"
    );

    let result = runner_task.await.unwrap();
    assert!(
        result.is_err(),
        "Expected runner to return Err after exhausting attempts"
    );
}

#[tokio::test]
async fn test_rdp_session_reconnect_user_cancellation() {
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("Skipping live endpoint test: network sandboxed ({e})");
            return;
        }
        Err(e) => panic!("Failed to bind loopback: {e}"),
    };
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let (ui_handle, worker_handle) = create_session_channels(32, 64);
    let mut config = SessionConfig::new("127.0.0.1", port, RemoteProtocol::Rdp);
    config.reconnect_policy = ReconnectPolicy {
        enabled: true,
        max_attempts: 5,
        initial_delay_ms: 1500, // 1.5s gives ample time for user cancellation
        max_delay_ms: 3000,
        backoff_multiplier: 2,
    };

    let fb = Framebuffer::new(800, 600).unwrap();
    let runner_task = tokio::spawn(async move { run_rdp_session(config, worker_handle, fb).await });

    let mut cancelled = false;

    while let Ok(event) = ui_handle.recv_event().await {
        match event {
            SessionEvent::StateChanged(SessionState::Reconnecting { .. }) => {
                // User clicks cancel / disconnect while in reconnecting backoff
                ui_handle
                    .try_send_command(SessionCommand::Disconnect)
                    .unwrap();
            }
            SessionEvent::StateChanged(SessionState::Disconnected) => {
                cancelled = true;
                break;
            }
            _ => {}
        }
    }

    assert!(
        cancelled,
        "Expected Disconnected state after user cancellation"
    );
    let result = runner_task.await.unwrap();
    assert!(
        result.is_ok(),
        "Cancellation should result in Ok(()) clean exit"
    );
}

#[tokio::test]
async fn test_rdp_session_disabled_reconnect_policy_exits_immediately() {
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("Skipping live endpoint test: network sandboxed ({e})");
            return;
        }
        Err(e) => panic!("Failed to bind loopback: {e}"),
    };
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let (ui_handle, worker_handle) = create_session_channels(32, 64);
    let mut config = SessionConfig::new("127.0.0.1", port, RemoteProtocol::Rdp);
    config.reconnect_policy = ReconnectPolicy::disabled();

    let fb = Framebuffer::new(800, 600).unwrap();
    let runner_task = tokio::spawn(async move { run_rdp_session(config, worker_handle, fb).await });

    let mut saw_reconnecting = false;
    let mut saw_disconnected = false;

    while let Ok(event) = ui_handle.recv_event().await {
        match event {
            SessionEvent::StateChanged(SessionState::Reconnecting { .. }) => {
                saw_reconnecting = true;
            }
            SessionEvent::StateChanged(SessionState::Disconnected) => {
                saw_disconnected = true;
                break;
            }
            _ => {}
        }
    }

    assert!(
        !saw_reconnecting,
        "Disabled policy must never enter Reconnecting state"
    );
    assert!(
        saw_disconnected,
        "Expected Disconnected state immediately on failure"
    );

    let result = runner_task.await.unwrap();
    assert!(
        result.is_err(),
        "Expected error return on failure with disabled reconnect"
    );
}

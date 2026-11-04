# Isolated OpenSSH integration test

The opt-in test exercises the GPUI app's PTY session worker and terminal grid
against an OpenSSH server. It checks rejection and acceptance of an unknown host
key, public-key authentication, reconnection with a known key, and rejection
of a changed key. It does not exercise GPUI rendering or keyboard events.

Run only against a temporary loopback server configured for this test. Do not
point it at a real host: the test intentionally responds to host-key prompts and
uses a private known-hosts file. It creates test-only files in a temporary
directory, forces a harmless command on the server, and does not change the
user's SSH configuration or known-hosts file. `sshd` must be able to start as
the test user; platform-specific server configuration may be needed.

For example, on a machine with OpenSSH client and server tools installed:

```sh
fixture=$(mktemp -d)
ssh-keygen -q -t ed25519 -N '' -f "$fixture/host_key"
ssh-keygen -q -t ed25519 -N '' -f "$fixture/client_key"
cat > "$fixture/sshd_config" <<EOF
Port 34567
ListenAddress 127.0.0.1
HostKey $fixture/host_key
PidFile $fixture/sshd.pid
AuthorizedKeysFile $fixture/client_key.pub
PubkeyAuthentication yes
PasswordAuthentication no
KbdInteractiveAuthentication no
UsePAM no
StrictModes no
PrintMotd no
UseDNS no
ForceCommand /bin/echo shellumi-auth-ok
EOF
/usr/sbin/sshd -D -e -f "$fixture/sshd_config" &
sshd_pid=$!
SHELLUMI_SSH_TEST_DIR="$fixture" SHELLUMI_SSH_TEST_PORT=34567 \
  cargo test --locked -p shellumi-app isolated_openssh_host_key_and_authentication -- --ignored
kill "$sshd_pid"
```

If port 34567 is in use, choose another high port in both the config and test
environment. Confirm the daemon is listening before running the test. Stop the
daemon and remove the test directory afterward. The fixture keys are disposable
and must not be used for actual host access.

# Isolated OpenSSH integration tests

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

## Saved identity and SFTP acceptance (macOS)

The self-contained acceptance test creates a temporary Turso database, reusable
public-key identity and group binding, an encrypted client key with spaces in its
path, and a separate temporary server host key. It starts `/usr/sbin/sshd` on an
available loopback port with `internal-sftp`, using disposable authorized keys
and no account password authentication. The test supplies the exact generated
host public key to a private known-hosts file; verification is never disabled.
An explicit OpenSSH config (`/dev/null`) isolates the test from user client config.

```sh
cargo test --locked --offline -p shellumi-app --test ssh_sftp_acceptance -- --ignored --nocapture
```

Run on macOS with OpenSSH client/server tools installed and permission to bind a
loopback socket. The test is opt-in because ordinary CI/development environments
may lack `sshd` or loopback permission. It does not skip failures when explicitly
run. Its daemon guard stops and waits for the temporary server on completion or
panic, and temporary keys, databases and files are removed afterward.

Verified behavior:

- Resolve a saved host's group identity after database reopen.
- Unlock the encrypted private key through the actual app executable in askpass
  mode using an environment reference to a disposable test passphrase.
- Negotiate SFTP v3, create/list/remove directories, and upload/download an exact
  131,079-byte binary payload across multiple SFTP packets.
- Transfer empty files and reconnect with the same identity.
- Report missing remote directories as failures.
- Refuse changed and unknown host keys, and an incorrect key passphrase.

The normal workspace suite also injects failed, mismatched and disconnected
CLOSE replies into the protocol client. Upload/download completion and directory
listing now require a matching successful CLOSE response; close-time disk/quota
errors cannot be reported as successful completion.

This establishes the graphical SFTP transport and helper behavior, not GPUI
button/focus/layout acceptance. Real password/MFA authentication, native keychain
access, cancellation during transfers, atomic destination replacement, and
Windows/Linux server/runtime coverage remain separate work. A failed transfer
can leave partial data at its destination; do not treat a failed task as a usable
complete file.

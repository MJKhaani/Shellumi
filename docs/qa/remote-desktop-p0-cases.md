# Remote desktop P0 case inventory

P0 acceptance is **NOT COMPLETE**. This inventory records cases awaiting full case-level execution and traceability. `NOT RUN` means the complete case specification has not been established by execution evidence; it does not imply that no narrower tests exist. Tests in the [acceptance record](remote-desktop-acceptance.md) cover selected portions only.

Source baseline: `0781e1f7c7191b9e8cb439b8bb110d78462939c6`; current working tree contains uncommitted changes. No tested commit or packaged artifact hash is asserted for these rows.

| Original case | Required behavior | Full-case status | Evidence and blocker |
| --- | --- | --- | --- |
| P01-T01 | In a real production-dispatch fixture, trigger recovery, receive matching ack, invoke toolbar Ctrl+Alt+Del; verify current envelope identity and protocol press/release sequence. Repeat for Alt+Tab and Windows key, on RDP and VNC. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P01-T02 | After recovery, exercise mapped Cmd+C/V/X/A/Z and ordinary typing; none is rejected due to an obsolete command variant. Distinguish synchronized paste from raw remote paste. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P01-T03 | Saturate queue while a shortcut is pending; recovery cancels the abandoned action, and a fresh post-recovery shortcut works. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P01-T04 | Ctrl physically held, invoke Ctrl+C, then release Ctrl; remote ledger and actual submitted events maintain the held state. Test Alt/Super conflicts and failure midway through a chord. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P01-T05 | All production input routes are validated; old/zero identity and legacy bypass attempts are rejected before submission. Test `CtrlAltDel` explicitly if still present. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P01-T06 | Local clipboard offer rejected, times out, or belongs to an old transport: synchronized paste does not send Ctrl+V or claim success. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P02-T01 | Fill the queue with non-coalescible presses, reject a release, trigger recovery; zero abandoned presses execute after the barrier, all submitted held keys/buttons are drained. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P02-T02 | Saturate with ordinary and non-input commands, request view-only/clipboard revoke; no independent epoch or default identity, no fail-open transition. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P02-T03 | Trigger two concurrent recoveries and deliver acknowledgements in reverse order; only current ticket may unblock input, no lost waiter. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P02-T04 | Interleave admission, queue pop, barrier publication and worker submission using deterministic synchronization; stale commands cannot cross the recovery linearization point. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P02-T05 | Saturate the event queue with frames/diagnostics; recovery and policy acknowledgements still arrive or the transport deterministically fails. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P02-T06 | Inject protocol send/write failure halfway through draining held state; no successful recovery ack, transport closes, fresh connection can accept input. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P02-T07 | Focus loss/window deactivation/tab switch with held keys and mouse buttons under load; release or safe transport termination, no stuck state. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P02-T08 | Recovery timeout, reconnect with pending old barrier, old ack arriving after reconnect, epoch exhaustion; no unlock or replay of stale input. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P02-T09 | Queue motion coalescing remains bounded and never crosses key/button/shortcut transitions; sustained traffic does not starve policy operations. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P03-T01 | Complete transfer A with `report.pdf`; start and cancel B with the same name. A's path, contents, SHA-256 and clipboard reference remain unchanged. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P03-T02 | Inject exclusive-create failure or a pre-existing foreign path during B; cancellation/failure cannot delete the foreign path. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P03-T03 | Complete one file then cancel a multi-file job; cleanup is confined to that job's own directory; unrelated completed jobs survive. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P03-T04 | Simulate symlink/reparse/root replacement and traversal attempts; no read/write/delete outside authorized root, no fallback. Run native Windows and Unix tests where possible. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P03-T05 | Cancel before SIZE, during RANGE, immediately before commit and after terminal completion; late responses cannot resurrect or mutate a terminal transfer. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P03-T06 | Two overlapping remote clipboard offers; old handles and stream IDs are invalidated or separately tracked, no lost terminal events. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P03-T07 | Empty, 8-byte, multi-chunk, duplicate-name, malformed and oversized files; actual SIZE quota, exact byte count and test hashes; no incomplete publication. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P03-T08 | Inject permission errors, disk full/write failure, rename collision, cleanup failure, request timeout and disconnect; one truthful terminal event; unrelated files remain intact. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P03-T09 | Staging creation failure followed by policy enable, view-only round trip and reconnect; no receive until a new validated root exists. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P03-T10 | Native clipboard retention and orphan cleanup: an active published download remains usable; orphan scavenging cannot remove unrelated paths. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P04-T01 | Authorize A, replace its original pathname with B, request data; only A's immutable authorized bytes or explicit failure, never B. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P04-T02 | Replace original path with symlink/reparse point to sentinel file after authorization; sentinel bytes never leave process. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P04-T03 | Modify/truncate/grow the source during and after staging; no mixed/extra bytes; snapshot or explicit failure. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P04-T04 | Remote requests beyond EOF, overflow, oversized request, malformed flags, wrong file index, stale lock/offer IDs; bounded allocation and correct failure. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P04-T05 | Revoke permission while snapshot is being created, while file is being read, and immediately before protocol submission; no content admitted after effective revoke. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P04-T06 | Send disabled with view-only false; direct worker and backend text/files commands cannot create or advertise offers. Format and RANGE requests cannot bypass the gate. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P04-T07 | Cancel active upload and request additional data; offer invalid, handles closed, safe terminal event, no stale bytes. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P04-T08 | Replace clipboard offer with a new one while old requests are pending; old request cannot read the new offer. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P04-T09 | Duplicate or out-of-order ranges, zero-byte file, short read and request/send failure; truthful accounting and no false remote completion. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P04-T10 | Reconnect during snapshot or upload; stale completion/response discarded, no new transport receives old content. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P05-T01 | Enable view-only with held mouse buttons/modifiers, active transfers, clipboard offer, authorized drive and active fake microphone; verify complete effective revocation, release sequence and matched ack. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P05-T02 | Inject key-release submit failure on RDP and socket-write failure on VNC; no success ack; transport is terminated and UI remains blocked. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P05-T03 | Saturate normal command/event queues during revoke; transition succeeds through reliable control path or closes transport. No fail-open state. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P05-T04 | Directly inject text/file staging, native publish callback, drive open/read/write/root change and microphone start/unmute while revoked; no prohibited operation reaches backend submission. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P05-T05 | Revoke only clipboard send/receive while interactive; independent policy enforced, active matching transfers cancelled and late responses rejected. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P05-T06 | Toggle on/off quickly; out-of-order/wrong-transport acknowledgements cannot overwrite newer state. Failure and timeout paths remain blocked. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P05-T07 | Leave view-only; keyboard/mouse restore only after current ack, drive/microphone do not silently regain permission. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P05-T08 | Reconnect while view-only and/or clipboard disabled; new backend never advertises or processes disallowed operations under defaults. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P05-T09 | Actual microphone stream fake asserts stop/drop, zero post-revoke callback sends and no restart until explicit consent. Include denied-device case. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P05-T10 | RDPDR fake/open handles: revoke closes or invalidates access, read-only and root policy remain intact, close does not leak permission. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P06-T01 | Queue every live command class for transport N, replace with N+1, deliver old work; no stale effect on N+1. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P06-T02 | Same tab, new connection generation with reused transport counter; stale command/event rejected. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P06-T03 | Different tab with identical counters; no cross-tab input, clipboard, file or settings access. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P06-T04 | Default/zero identity for all live command classes, including recovery and CtrlAltDel; rejected, no wildcard. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P06-T05 | Old file-picker result after reconnect/close, old snapshot and transfer completion; no transfer or native clipboard write on new session. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P06-T06 | Late frame, resize ack, policy ack, recovery ack and metrics events from retired worker; current state unchanged. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P06-T07 | Auto-reconnect during queued input and active transfer; old work cancelled, new legitimate input works after handshake, no stale replay. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P06-T08 | Old cancellation and disconnect after new transport starts; new transport unaffected. Explicit whole-session close still works. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P06-T09 | Reconnect while view-only, clipboard disabled or mic denied; restrictions effective before backend starts. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |
| P06-T10 | New transport receives legitimate new clipboard offer, shortcut, resize and permitted settings; no accidental permanent rejection from old epoch state. | NOT RUN | Full production-path, protocol and platform evidence not yet mapped or executed. |

## Focused closure cases

Each focused case also requires its specified production-path assertions and execution evidence. The items below are an inventory, not a completed mapping to the original cases.

| Case | Full-case status | Evidence and blocker |
| --- | --- | --- |
| C1-01 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C1-02 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C1-03 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C1-04 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C1-05 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C1-06 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C2-01 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C2-02 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C2-03 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C2-04 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C3-01 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C3-02 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C3-03 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C3-04 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C3-05 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C3-06 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C4-01 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C4-02 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C4-03 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C4-04 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C4-05 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C4-06 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C4-07 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C4-08 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C4-09 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C5-01 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C5-02 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C5-03 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C5-04 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C5-05 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C5-06 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C5-07 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C5-08 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C6-01 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C6-02 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C6-03 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C6-04 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C6-05 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C6-06 | NOT RUN | Full case-level production-path execution and evidence not yet established. |
| C6-07 | NOT RUN | Full case-level production-path execution and evidence not yet established. |

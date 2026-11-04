# Feature status

This is an initial coarse inventory; expand into individual acceptance criteria
before implementing each area. Library code alone is not a complete user feature.

| Feature | Phase | Status | Evidence / next test |
| --- | --- | --- | --- |
| GPUI desktop window | 0 | Blocked | macOS build requires an available Metal shader compiler; build and run on each desktop OS |
| Security architecture | 0 | In progress | See security/threat-model.md; review before implementing secrets |
| SSH and host verification | 1 | Not started | Select a backend; verify unknown, changed and revoked keys against a real server |
| Local terminal | 1 | Not started | Integrate a PTY and terminal engine; test interactive applications |
| Host storage and credentials | 2 | In progress | Turso host metadata storage has tests; UI, backup and credential storage are outstanding |
| Workspaces and terminal | 3 | Not started | Interactive terminal compatibility and layout tests |
| SFTP, tunnels, proxies | 4 | Not started | Integration tests with OpenSSH |
| Encrypted Google Drive sync | 5 | Not started | Threat model, encryption and conflict tests |
| Snippets and automation | 6 | Not started | Command preview and injection tests |
| Mosh, Telnet, serial | 7 | Not started | Protocol-specific integration tests |
| Collaboration | 8 | Not started | Authorization and cryptographic review |
| Infrastructure integrations | 9 | Not started | Provider fixtures and permission tests |
| Release hardening | 10 | Not started | Cross-platform builds, security review, packaging |
| Mobile feasibility | 11 | Not started | Validate GPUI and platform lifecycle support |

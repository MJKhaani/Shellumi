# Mechanics investigation

The original is an Electron app. Its launcher (`Contents/MacOS/Termius`, SHA-256 `de0da0ef17fbe98cb9200f05610d5e6327332b245170312dd601b174fa68c27b`) did not yield matches for host, port, password, connection, or settings. Its bundled `@termius/libtermius/mac-x64/termius.node` (SHA-256 `4e45d032d0b12b57d4443bb619d3480e9c13e887f1aa8f9baa90cce51b802013`) contains the native SSH engine.

## Observations

- The password string at `0x2451e8` is referenced by the Node SSH connection handler at `0xc0f0` (reference at `0xc477`; evidence `ev_415fa4b9a978587f383f54ecaa06cf9de820505c18f83a87d7b456e3635f157e`).
- The handler reads connection properties including `username`, `password`, `keys`, `host`, `port`, `knownhosts`, `keepaliveInterval`, and `enableCompression`, and passes options to `SshClient::Connect` (evidence `ev_5453fbe133758c7455ad34239a0f7787aca497a2202a7fe2973cf6f8d7ff9024`).

## Limits and open work

Ghidra failed to decompile the handler. The fallback output is register-level, with 138 unstructured jumps and 151 untranslated instructions; it is insufficient to establish the complete rules or reliably attribute defaults. The UI and additional logic are also in the ASAR's JavaScript. No formulas or behavior changes have been asserted from this incomplete analysis. Further tracing and successful decompilation are needed before claiming equivalence.

## Additional connection trace

The main executable was reopened in Deep mode. Searches for host, port, password, settings and connection returned no strings; the search examined zero strings, so this is not proof of absence (for example, evidence `ev_e27d9fa14c5e3faf29b77aa13d7f17de72c6a45c2490b07ea768b8fb7f9b376a`). The native module's `keepaliveInterval` and `enableCompression` strings refer to the connection handler at `0xc0f0` (evidence `ev_15d6ebd3131bd24ae725335d8495e0e0aaa36244888b5f1b29ba4944c8b57c23` and `ev_f1af76018c1c9b53760a95827f88ff8451843652ca188774164b61499f839225`).

- `NodeSshClient::Connect` at `0xc0f0` calls `SshClient::Connect` at `0xf4200`, which calls `SshClientPimpl::Connect` at `0xf4280`; that calls `SshClientPimpl::CheckOptions` at `0xf8030` (evidence `ev_04a5e19a7c118da91f0c9b46ba6d7b86db985208cf257789cc2557eb1f19b26f`, `ev_8c0d186a5a5628723e68f99a00537f44219426e9bfe4feca18d6d3bf97aafd64`, `ev_a569cbc8e7449fc94755376886b4fff1c0ae08ad1ac3bc8b86f03a93d4be4ccb`).
- The fallback for `CheckOptions` indicates that a direct connection with an empty host fails, and that a configured proxy fails when its host or port is empty (evidence `ev_ec710fd4cff364251cd140d3b2150ddcc1ec067177b3dfd72373d7ef7078c2d1`). These are native validation observations, not proof of the UI's input rules or whitespace handling.
- Ghidra again returned no result for all four functions. The fallback uses registers and reconstructed control flow, so it does not establish complete defaults or formulas. No unverified values have been substituted into the build.

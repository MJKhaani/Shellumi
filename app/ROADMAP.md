# Shellumi roadmap

The current application is an early Harbor prototype, not Shellumi or a production SSH client. It uses egui rather than the required GPUI and does not yet satisfy the architecture or feature requirements in PROJECT.md.

Next: research current GPUI and dependency APIs, design a modular Rust workspace and threat model, and replace the prototype UI with a buildable GPUI shell. Then implement and test a usable terminal and secure SSH connection flow before proceeding through the remaining phases in PROJECT.md. No cross-platform support is claimed until tested on each platform.

Current prototype: one local or OpenSSH session, saved host metadata and line-based input. Host verification asks for confirmation through OpenSSH; credentials are not saved. Terminal interaction, host key management, secure credential storage, synchronization, and the other roadmap features remain incomplete.

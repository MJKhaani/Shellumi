# <img src="../../resources/icon_pack/icons/48/regular/snippets.png" width="24" height="24" alt="Snippets"> Snippets

[Docs Home](../README.md) / [User Guide](README.md) / Snippets

---

Snippets store reusable multi-line shell commands with optional template variables (`{{var:-default}}`).

- **Create & Edit:** Use the Snippets page or Snippet Inspector to define command text, description, and tags.
- **Run Safely:** Running a snippet opens a parameter/confirmation dialog before sending text to a terminal; startup snippets are only displayed/selected explicitly and never execute silently on untrusted hosts.
- **Batch Execution:** Run a snippet across multiple hosts sequentially or in parallel via `shellumi-cli snippet batch` or the desktop batch modal.
- **Import / Export:** Export or import snippet collections as JSON.

# <img src="../../resources/icon_pack/icons/48/regular/infrastructure.png" width="24" height="24" alt="Infrastructure"> Infrastructure & Ansible

[Docs Home](../README.md) / [User Guide](README.md) / Infrastructure

---

The **Integrations** page and Provider Inspector manage local infrastructure provider records and Ansible inventories:

- **Provider Records:** Track AWS, Azure, DigitalOcean, or Ansible inventory sources locally via the unified Provider Inspector.
- **Ansible Import / Export:** Import INI or YAML Ansible inventories into hosts/groups (non-destructive merge preserving existing user tags and notes) or export saved hosts back to Ansible format.

> [!NOTE]
> Authenticated live cloud API polling (AWS/Azure/DigitalOcean) is not yet enabled in the desktop UI; provider records currently organize local/imported inventory metadata.

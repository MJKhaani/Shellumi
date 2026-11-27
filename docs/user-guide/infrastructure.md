# <img src="../../resources/icon_pack/icons/48/regular/infrastructure.png" width="24" height="24" alt="Infrastructure"> Infrastructure Discovery & Inventory

[Docs Home](../README.md) / [User Guide](README.md) / Infrastructure

---

Shellumi provides file- and CLI-based infrastructure inventory parsers (`AwsEc2Parser`, `AzureVmParser`, `DigitalOceanParser`, `AnsibleInventoryEngine`) to import cloud and automation inventories into your local vault without overwriting existing custom host settings.

> [!NOTE]
> **File-Based Parsing vs. Live Cloud Polling:**
> Current support parses **exported JSON/inventory files** (`infra discover ... --file`) and **Ansible INI/YAML** files (`infra ansible`). Live background AWS/Azure/DigitalOcean API polling and external audit connectors (such as Vanta) remain **Planned** (`REQ-INFRA-001`..`004`).

---

## Supported Providers & Workflows

- **GUI:** Open **Integrations** (via Command Palette or Vaults/Integrations route) to manage saved `CloudProviderRecord` entries and discovered hosts in the Inspector.
- **CLI:** Use `shellumi-cli <database-path> infra ...` to add/sync provider records, discover from exported files, and import/export Ansible inventories.

### Practical Examples (Sanitized)

```bash
# Parse an exported AWS EC2 JSON inventory and import discovered hosts
shellumi-cli ~/vault.db infra discover aws --file ec2-instances.json --import-all

# Parse Azure VMs preferring private VPC/VNet IP addresses
shellumi-cli ~/vault.db infra discover azure --file azure-vms.json --private-ip

# Parse DigitalOcean Droplets
shellumi-cli ~/vault.db infra discover digitalocean --file droplets.json

# Import / export Ansible inventory (INI or YAML)
shellumi-cli ~/vault.db infra ansible import --file hosts.ini --format ini
shellumi-cli ~/vault.db infra ansible export --format yaml --output inventory.yml
```

Example sanitized Ansible INI (`hosts.ini`):

```ini
[web]
web-01 ansible_host=203.0.113.10 ansible_user=deploy ansible_port=22
web-02 ansible_host=203.0.113.11 ansible_user=deploy ansible_port=22
```

---

## Collision Handling, Credentials & Troubleshooting

- **Collision & Customization Safety:** Re-importing inventories preserves local host customizations rather than blindly overwriting credentials.
- **Credential Hygiene:** Provider API tokens configured via `infra provider add --token <t>` are stored encrypted and excluded from plaintext diagnostics (prefer avoiding `--token` on shared hosts per [`SEC-001`](../project/known-issues.md#platform-security--operational-gaps)).
- **Common Errors:**
  - *Parse Error:* Ensure AWS/Azure/DO files match standard CLI JSON output (`aws ec2 describe-instances`, `az vm list`, `doctl compute droplet list --output json`).

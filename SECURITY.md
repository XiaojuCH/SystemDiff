# Security policy

## Current support status

SystemDiff has no public release yet. The bootstrap code and draft schema do not carry a production security-support promise. Once releases begin, the project intends to support the latest release line unless a release note states otherwise.

## Reporting a vulnerability

Do not include vulnerability details, real snapshots, task XML, tokens, personal data, or sensitive logs in a public issue.

The public repository must enable GitHub Private Vulnerability Reporting before launch. Until a private channel is published, use a private channel you already have with the maintainer. If none exists, open a minimal public issue asking the maintainer to establish private contact, without disclosing technical details.

Include, when safe:

- affected version/commit and Windows version;
- privilege/elevation state;
- impact and realistic attacker prerequisites;
- minimal reproduction using synthetic data;
- relevant Collector/schema/rule identifiers;
- suggested mitigation if known.

The project does not promise a fixed response SLA during bootstrap. Maintainers will acknowledge reports when capacity permits and coordinate disclosure after impact and a fix path are understood.

## Sensitive report handling

Snapshots and reports may contain usernames, paths, software, network details, hashes, task definitions, host information, or secrets embedded by other programs. The sanitizer is not implemented. Treat every report as sensitive and manually inspect it before sharing.

## Defensive project boundary

In scope:

- inspecting documented persistence and configuration locations;
- hashing and signature inspection;
- comparing versioned evidence;
- identifying and explaining suspicious changes;
- mapping findings to documented defensive concepts.

Out of scope:

- credential, token, cookie, or secret extraction;
- keylogging or surveillance;
- creating persistence;
- AV/EDR bypass, stealth, evasion, RAT, or C2 functionality;
- automated exploitation or unauthorized access;
- automatic cleanup/remediation in the MVP.

A proposal crossing this boundary requires the maintainer to stop, reassess scope, and update the threat model before implementation.

## Security design references

See [product principles](docs/product-principles.md), [architecture](docs/architecture.md), and the [threat model](docs/threat-model.md).

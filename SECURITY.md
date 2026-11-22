# Security policy

## Reporting a vulnerability

Please report vulnerabilities privately via GitHub's
[private vulnerability reporting](https://github.com/waschmittel/tct/security/advisories/new)
rather than a public issue. Expect an acknowledgement within a few days.

## Threat model

`tct` treats everything under a data directory as untrusted, because a
discovered `.tct` may come from a checked-out repository or a shared directory.
Ids are validated, symlinks are never followed on read, writes use fresh
exclusive temp files, a discovered `.tct` must be owned by you and not
world-writable, and terminal output is sanitized. See `AGENTS.md` ("Untrusted
data dir") and `docs/docker/README.md` for the web deployment.

## Release artifacts

Release binaries carry signed build provenance and are listed in `SHA256SUMS`:

```sh
gh attestation verify tct-<version>-<platform> --repo waschmittel/tct
```

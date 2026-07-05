# Security Policy

## Reporting A Vulnerability

Please report security issues privately before opening a public issue.

Send a concise report with:

- affected version or commit
- operating system and Rust version
- minimal reproduction input, when possible
- expected impact

If no private advisory channel exists for the current repository host, email
the repository owner listed on the project page and avoid attaching untrusted
font files to public tickets.

## Scope

Security-sensitive areas include:

- TrueType table parsing
- glyph loading and composite expansion
- bytecode interpreter state
- rasterizer bounds checks
- fixture/oracle tooling that handles untrusted font files

The runtime crate forbids unsafe Rust and runtime FreeType FFI. Do not bypass
those guards to work around a security issue.

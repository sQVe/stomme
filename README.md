# stomme

A shared foundation for project structure, coding rules, and documentation.

stomme will provide versioned rules, project skeletons, and a Rust command-line tool to keep
projects in sync. Each project keeps its own modules and decisions. Drift from the shared rules
will fail the project's full check.

stomme is in early development. `stomme check` checks the project in the current directory: its
`stomme.toml`, its required files, and its ADR index.

See [the documentation](docs/README.md) for the development guide and architecture decisions.

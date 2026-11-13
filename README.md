# stomme

A shared foundation for project structure, coding rules, and documentation.

stomme will provide versioned rules, project skeletons, and a Rust command-line tool to keep
projects in sync. Each project keeps its own modules and decisions. Drift from the shared rules
will fail the project's full check.

stomme is in early development. The binary is a stub that prints usage to stderr and exits 1;
no commands are available yet.

See [the documentation](docs/README.md) for the development guide and architecture decisions.

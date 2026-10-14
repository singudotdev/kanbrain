# Contributing

Thanks for helping out! Everything you need to build, test and understand the code is in
[docs/development.md](docs/development.md).

In short:

```sh
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
KANBRAIN_DIR=/tmp/kanbrain-dev cargo run -- add --start "Try it"   # the command line, without touching real data
KANBRAIN_DIR=/tmp/kanbrain-dev cargo run -p kanbrain-app           # … and the kanban app
```

The shared logic is in `core/`, the command line in `cli/` and the kanban app in `gui/`. Plain
`cargo test` skips the kanban app (it needs GPUI and, on Linux, some graphics libraries: see
[docs/development.md](docs/development.md)).

- Keep pull requests focused, and describe the behavior change in the description.
- Add tests to the crate's `tests/` folder (see [Tests](docs/development.md#tests)): one that fails without the
  change, and for every behavior the change could affect. CI's mutation testing fails on a changed line no test
  notices. If you change what agents are told (the `init` section, `context`, the hooks), run `e2e/agent.sh` too;
  if you change an installer, `e2e/install.sh` (CI runs it, and `e2e/install.ps1`, on every push).
- Update `docs/` when keys or behavior change.
- Contributions are licensed under the project's license, [AGPL-3.0-only](LICENSE) with the additional terms in
  [NOTICE](NOTICE).
- File format changes: see [Changing the file format](docs/development.md#changing-the-file-format). Data folders
  written by a released version must keep loading.

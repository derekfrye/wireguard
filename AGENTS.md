# Repository Guidelines

## Project Structure & Module Organization
- `rust-wg/` is the Rust workspace; application code lives in `rust-wg/src/`.
- `docs/` contains the clean-room spec and operational notes (e.g., `docs/wg_spec.md`, `docs/test_plan.md`).
- `examples/` holds helper assets such as the dev-container unit (`examples/rust-wg-dev.container`).
- `examples/Dockerfile` builds the container image used by the dev plan.

## Build, Test, and Development Commands
- `cargo build` (run from `rust-wg/`): compile the runtime binary.
- `cargo run -- run` (from `rust-wg/`): start the runtime using default config discovery.
- `cargo run -- generate` (from `rust-wg/`): generate configs without starting the runtime.
- `podman build -t localhost/djf/rust-wg -f examples/Dockerfile .`: build the dev container image (see `docs/test_plan.md`).
- `systemctl --user start rust-wg-dev.container`: start the dev container unit.

## Coding Style & Naming Conventions
- Rust 2024 edition; keep modules in `snake_case`, types in `PascalCase`.
- Prefer explicit, readable naming for networking and config fields.
- No repo-specific formatter or linter config is checked in. Use `cargo fmt`/`cargo clippy` if the environment provides them, but don’t add tooling changes unless requested.

## Testing Guidelines
- No automated test suite is present yet.
- Use the manual dev-container plan in `docs/test_plan.md` for verification (WireGuard interface, config generation, and host handshake).
- When adding tests, keep names descriptive (e.g., `config_parsing_*`) and document the run command.

## Commit & Pull Request Guidelines
- Commit messages in history are short, lowercase, and imperative (e.g., “ready to test”, “api changes”).
- Keep commits focused; note any dev-container or config changes explicitly.
- PRs should include: a brief summary, testing notes (or a reason tests were skipped), and links to any related issues/spec updates in `docs/`.

## Security & Configuration Notes
- The runtime config contract is defined in `docs/wg_spec.md`; changes here should precede code changes.
- Runtime data is expected under `/var/lib/wg` inside the container.

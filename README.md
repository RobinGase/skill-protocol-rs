# skill-protocol

[![CI](https://github.com/RobinGase/skill-protocol-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/RobinGase/skill-protocol-rs/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

A small Rust crate that gives Claude Code skill authors a working
foundation for skill CLIs:

- A JSON request/response protocol so an orchestrator can call your
  skill over stdin/stdout.
- Path helpers for resolving the workspace root and skill folder.
- A `.env` loader so skills hydrate their own environment without a
  heavy dotenv crate.
- A trait-based config seam (`SkillConfig`) so your skill stays
  portable across hosts.
- A self-contained CSV + XLSX exporter for skills that drop tabular
  artefacts on disk.

This crate powers [`leads-skill`](https://github.com/RobinGase/leads-skill)
and [`gmail-skill`](https://github.com/RobinGase/gmail-skill); use it
directly when building your own.

## Install

```toml
[dependencies]
skill-protocol = { git = "https://github.com/RobinGase/skill-protocol-rs" }
```

Or pin a commit:

```toml
skill-protocol = { git = "https://github.com/RobinGase/skill-protocol-rs", rev = "<sha>" }
```

## Quick example

```rust
use serde_json::Value;
use skill_protocol::{
    SkillCliResponse, hydrate_env_from_workspace, read_request, write_response,
};

fn main() {
    let response = match run() {
        Ok(response) => response,
        Err(error) => SkillCliResponse::blocked(error, Value::Null),
    };
    let _ = write_response(&response);
    if !response.ok {
        std::process::exit(1);
    }
}

fn run() -> Result<SkillCliResponse, String> {
    let request = read_request()?;
    hydrate_env_from_workspace(&request)?;

    match request.command.trim() {
        "status" => Ok(SkillCliResponse::ok(
            "ready",
            "skill is wired up",
            Value::Null,
        )),
        other => Err(format!("unsupported command '{}'", other)),
    }
}
```

The same binary can be wrapped behind a clap-based CLI for direct human
or Claude Code invocation. See the leads-skill and gmail-skill repos for
worked examples.

## What's in the box

| Module | Purpose |
|---|---|
| `protocol` | `SkillCliRequest`, `SkillCliResponse`, `read_request`, `write_response`, `parse_settings` |
| `paths` | Workspace/skill-root resolution, `.env` loader, `hydrate_env_from_workspace` |
| `config` | `SkillConfig` trait + `BasicSkillConfig` env-driven default |
| `report` | `export_report_artifacts` (CSV + XLSX), `ReportExportResult`, `display_artifact_path` |
| `tool` | `NativeToolStatus`, `ToolRunRequest`, `ToolRunResponse`, `GmailComposeRequest`, `GmailMessageSummary` |

## Design philosophy

- **No async runtime.** Skills decide. The crate is sync-only and
  exposes plain functions; build async on top if you need it.
- **No heavy deps.** `serde`, `serde_json`, and `zip` (for XLSX). That's
  it.
- **Small surface.** The protocol intentionally has a single command +
  optional operation pair so simple skills stay simple.

## Status

`0.1.x` — the protocol is stable enough that two production skills
build on it, but minor cleanups may land. Pin a commit if you don't
want surprises.

## Contributing

Issues and PRs welcome. Keep changes scoped, add tests where the
behaviour is observable, and run `cargo fmt && cargo clippy --
-D warnings && cargo test` before sending.

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.

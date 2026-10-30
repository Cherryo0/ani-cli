# AGENTS.md — ani-cli

Agent operating manual for this repo. Read this before touching anything.
The human workflow (build, test, release, push) lives here; per-module API
docs live in `docs/`.

## What this is

`ani-cli` — a Rust TUI anime watcher modeled on MovieBox-Tui's look and
keys. Reverse-engineered providers (HiAnimeX, HiAnimeAt, WatchAnimeWorld,
AnimoStream), direct HLS extraction from ZokoAnime embeds, mpv/vlc/iina
playback with subtitles. Binary name and repo: `Cherryo0/ani-cli`.

**Naming collision warning:** a famous shell project is also called
`ani-cli` (pystardust/ani-cli). Ours is a distinct Rust TUI. The user has
not been warned about this; don't bring it up unprompted.

## Environment

- Rust 1.99, ratatui 0.29, crossterm, clap, tokio, reqwest (rustls).
- Dev VM: `~/workspace/ani-cli`. PATH needs `$HOME/.cargo/bin`
  (or `export PATH="$HOME/.cargo/bin:$PATH"`).
- TLS quirk on the dev VM: egress proxy MITMs TLS. `providers::http_client()`
  loads extra roots from the `SSL_CERT_FILE` env var. Plain
  `reqwest::Client::new()` **fails** here — always use `http_client()`.
- User's Mac: macOS arm64, Homebrew at /opt/homebrew, installs via the
  in-repo tap formula.

## Build / test

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build
cargo test                    # unit + render tests (fast, offline)
cargo test -- --ignored       # live smoke tests (hits real sites, ~10s)
cargo run -- --provider hianimeat   # manual run
```

- 46 unit + 10 TestBackend render tests (`tests/ux_render.rs`) must pass.
- `tests/live_smoke.rs` has 5 ignored live tests: hianimex search/episodes,
  hianimex server chain, zokoanime extractor, watchanimeworld search,
  **hianimeat full chain** (asserts direct HLS first, subtitle present, and
  the m3u8 serves a valid `#EXTM3U` playlist — this is the end-to-end
  playback proof).
- Render tests catch UI regressions: a mutation test proved renaming
  "Languages:" or changing `[hls]` fails exactly the right tests.
- Keymap tests in `src/app.rs` use `with_temp_home` — history tests must
  never touch the real `~/.config/ani-cli/`.

## Release procedure (do exactly this, in order)

1. Bump `version` in `Cargo.toml`.
2. `cargo build` (sanity), `cargo test` (all green).
3. Commit: `git add -A && git commit -m "<what changed>"`.
4. Tag: `git tag -a vX.Y.Z -m "vX.Y.Z: <what>"`.
5. Push **only via SSH** (port 22 is blocked; the GitHub app connector
   403s on pushes to this repo):
   ```bash
   export GIT_SSH_COMMAND='ssh -i /home/hatch/.ssh/github_cherryo0 -o UserKnownHostsFile=/home/hatch/.ssh/known_hosts'
   git push ssh://git@ssh.github.com:443/Cherryo0/ani-cli.git main vX.Y.Z
   ```
6. Wait ~5s, download the tag tarball, take its sha256:
   ```bash
   curl -sL --max-time 90 "https://github.com/Cherryo0/ani-cli/archive/refs/tags/vX.Y.Z.tar.gz" -o /tmp/ani-cli-vX.Y.Z.tar.gz
   sha256sum /tmp/ani-cli-vX.Y.Z.tar.gz | cut -d' ' -f1
   ```
7. Update `Formula/ani-cli.rb`: new tag URL + sha256. Commit "Formula: bump
   to vX.Y.Z". Push main again (SSH, same as step 5).
8. **Force-push `main` → `probe-test`** (same SSH remote). The repo's
   default branch is still `probe-test` (user hasn't flipped it in Settings
   yet) and `brew tap` clones the default branch — without this, Homebrew
   users get stale code:
   ```bash
   git push --force ssh://git@ssh.github.com:443/Cherryo0/ani-cli.git main:probe-test
   ```
9. Tell the user: `brew update && brew upgrade cherryo0/ani-cli/ani-cli`.

The formula lives **in this repo** at `Formula/ani-cli.rb`. Install needs
the three-part name: `brew install Cherryo0/ani-cli/ani-cli`. The separate
`~/workspace/homebrew-anime` tap repo is **abandoned** — never touch it.

## Conventions

- Providers are one file each in `src/providers/`, all through the
  `Provider` trait in `src/providers/mod.rs`. Adding a site = one new file
  + register in `all_providers()`. Keep providers self-contained (don't
  share parsing helpers across files — if one site changes markup, the
  others keep working).
- UI parity target is MovieBox-Tui (cloned at `/tmp/moviebox-tui` for
  reference): Catppuccin Mocha palette, block-letter logo, centered search
  bar with provider pill, peach `[key]` hints, type-to-search on Home,
  `q` backs out (quits only on Home), Ctrl+P cycles provider.
- Home screen shows **Recently played** (watch history), never auto-fetched
  trending. Trending is on-demand via the `/trending` slash command;
  `/history` jumps back.
- Direct/HLS sources sort above `EmbedPage` fallbacks so the default pick
  plays in mpv, not the browser.
- `StreamSource.subtitle_url` feeds mpv `--sub-file` / vlc `:input-slave` /
  iina `--mpv-sub-files`.
- Never create GitHub repos via browser on the user's behalf — the user
  creates empty repos themselves; we push over SSH. (Applies to all repos,
  not just this one.)

## Known warts

- Repo default branch is `probe-test`; flip to `main` needs the user's click
  in Settings → General (remind, don't nag).
- AnimoStream provider is WAF-blocked in practice (Cloudflare 403 on cold
  requests); WatchAnimeWorld sources are all EmbedPage (browser-only).
  Only HiAnimeX and HiAnimeAt give direct in-player streams.
- Swift/macOS compile of anything here is hand-reviewed only; the dev VM is
  Linux.

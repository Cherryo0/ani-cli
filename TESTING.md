# TESTING.md — ani-cli test checkpoints

Run through these 10 checkpoints before every release. Each checkpoint states
**what to do**, **what success looks like**, and the **command / keys** to use.

> Automated unit tests cover parsing + player command construction:
> `cargo test`. The checkpoints below are the manual end-to-end pass.

---

## CP-1 — Clean build from scratch
**Do:** `cargo clean && cargo build --release`
**Success:** Build finishes with zero errors and zero warnings.
**Why:** Catches missing deps / edition issues on a fresh machine.

## CP-2 — Unit tests pass
**Do:** `cargo test`
**Success:** All tests pass, including:
- `providers::tests::parses_dual_audio_title` — "Dual/Multi Audio [Hindi ORG, ENG]" → `[Hindi, English]`
- `providers::tests::parses_japanese_and_dedupes`
- `player::tests::mpv_gets_referer_header`
- `player::tests::from_name_roundtrip`

## CP-3 — App boots to the home screen
**Do:** `cargo run` (or `./target/release/ani-cli`)
**Success:** TUI opens, header shows `ani-cli`, provider tabs
(AnimoStream / WatchAnimeWorld / HiAnimeX), search box, and a trending list
populates within ~10s. No panic, `q` quits cleanly restoring the terminal.

## CP-4 — Search finds a known anime
**Do:** Press `/`, type `naruto`, press `Enter`.
**Success:** Results list appears with the header switching to "Results",
status bar shows `N result(s) for "naruto"`. `Esc` unfocuses the search box.

## CP-5 — Provider switching works (Tab)
**Do:** On the home screen press `Tab` twice.
**Success:** Active provider tab highlights each press, trending list reloads
from the new provider, `is_search_results` resets. No stale results shown.

## CP-6 — Detail screen shows languages BEFORE episodes
**Do:** `Enter` on any result.
**Success:** Detail screen shows the title, a **Languages:** line
(e.g. `Hindi · English`) parsed from the release title, synopsis/genres, and
the episode list below it. `b` goes back to results.

## CP-7 — Episode → server list resolves playable streams
**Do:** `Enter` on an episode.
**Success:** Sources screen lists servers with kind badges (`[file]` / `[hls]`
/ `[browser]`) and quality labels (`720p`, …). Empty source lists show
`Error: No playable sources found for this episode` instead of hanging.

## CP-8 — Player cycling + auto-detect
**Do:** Press `p` repeatedly on any screen.
**Success:** Header `player:` cycles mpv → vlc → iina → ffplay → celluloid →
haruna. Players not installed show `<name> not installed` in the status bar.
On boot with no `--player` flag, the first *installed* player is auto-picked.

## CP-9 — Playback hands off and returns cleanly
**Do:** Pick a `[file]`/`[hls]` source, press `Enter`.
**Success:** TUI suspends (terminal restored), the external player opens the
stream with the episode title, and when the player exits the TUI resumes
exactly where it was. Status bar shows `Finished: <title>` or a clear
`Player error: …` — never a stuck/half-drawn screen.

## CP-10 — CLI flags behave
**Do:**
- `ani-cli --player vlc` → header shows `player: vlc`
- `ani-cli --provider hianimex` → HiAnimeX tab active on boot
- `ani-cli --player nonsense` → falls back to auto-detect, no crash
- `ani-cli --help` → prints usage and exits 0

---

### Regression notes
- If a provider's site changes layout and CP-4/CP-7 start failing, the fix
  belongs in `src/providers/<site>.rs` only — the UI and player layers are
  site-agnostic by design (see `Provider` trait in `src/providers/mod.rs`).

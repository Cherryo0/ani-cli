# ani-cli

Watch anime in your terminal. A Rust TUI in the spirit of
[MovieBox-Tui](https://github.com/mesamirh/MovieBox-Tui) — search, browse,
pick an episode, pick a server, and it hands the stream to your favourite
video player.

## Providers

| Provider | Site | Notes |
|---|---|---|
| HiAnimeX | hianimex.com.in | **Default.** SUB/DUB servers; ZokoAnime embeds are cracked to **direct HLS** → plays straight in mpv |
| WatchAnimeWorld | watchanimeworld.one | WordPress; per-language dub-player sources (Hindi/Tamil/Telugu/English/Japanese) |
| AnimoStream | animostream.xyz | Hindi-dubbed index (Blogger). Note: its Cloudflare WAF often blocks automated requests — switch with `Tab` if it errors |

Press `Tab` to switch providers any time.

## Playback

Sources tagged **[Direct]** hand an HLS/MP4 URL straight to your player
(mpv recommended). Sources tagged **[browser]** are embed pages the app
opens in your web browser as a fallback.

## Video players

Auto-detected in this order: **mpv** → vlc → iina → ffplay → celluloid → haruna.
Press `p` inside the app to cycle, or force one:

```sh
ani-cli --player vlc
```

mpv is recommended — it handles direct files, HLS streams and custom
`Referer` headers out of the box.

## Install

```sh
# from source
cargo install --git https://github.com/Cherryo0/ani-cli

# macOS (Homebrew tap)
brew tap cherryo0/anime
brew install ani-cli
```

## Usage

```sh
ani-cli                          # auto-detect player, AnimoStream first
ani-cli --provider hianimex      # start on HiAnimeX
ani-cli --player mpv             # force mpv
```

### Keybindings

| Key | Action |
|---|---|
| `/` | focus search |
| `Enter` | open / play |
| `Tab` | switch provider |
| `p` | switch video player |
| `b` / `Esc` | back |
| `↑` `↓` / `j` `k` | move |
| `?` | help |
| `q` | quit |

Language tags (e.g. `[Hindi, English]`) are parsed from each release title
and shown **before** you pick an episode, so you know the dub/sub options
up front.

## Testing

See [TESTING.md](TESTING.md) — 10 checkpoints, plus `cargo test`.

## License

MIT — for personal/educational use.

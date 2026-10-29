use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "ani-cli", version, about = "Watch anime in your terminal")]
struct Cli {
    /// Video player to use (mpv, vlc, iina, ffplay, celluloid, haruna).
    /// Auto-detected if omitted.
    #[arg(short, long)]
    player: Option<String>,

    /// Provider to start with (hianimex, watchanimeworld, animostream).
    #[arg(short = 'P', long)]
    provider: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    ani_cli::app::run(cli.player, cli.provider).await
}

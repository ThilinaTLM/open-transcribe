use anyhow::Result;
use clap::Parser;

use open_transcribe::cli::{Cli, Commands};
use open_transcribe::client::run_client;
use open_transcribe::config::ClientConfig;
use open_transcribe::download::download_model;
use open_transcribe::server::run_server;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { host, port } => {
            println!("🎵 Open Transcribe Server");
            println!("========================");
            run_server(host, port)
                .await
                .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;
        }
        Commands::Download { model, models_path } => {
            println!("📥 Downloading Whisper Model");
            println!("============================");
            download_model(&model, models_path).await?;
        }
        Commands::TranscribeFile {
            audio_file,
            server_url,
            sample_rate,
            channels,
            bit_depth,
        } => {
            let config = ClientConfig::new_file_mode(
                server_url,
                audio_file,
                sample_rate,
                channels,
                bit_depth,
            );
            run_client(config).await?;
        }
        Commands::Record {
            duration,
            server_url,
            sample_rate,
            channels,
            bit_depth,
        } => {
            let config = ClientConfig::new_record_mode(
                server_url,
                sample_rate,
                channels,
                bit_depth,
                duration,
            );
            run_client(config).await?;
        }
    }

    Ok(())
}

use anyhow::{anyhow, Result};
use std::path::Path;
use std::process::Command;

const AVAILABLE_MODELS: &[&str] = &[
    "tiny",
    "tiny.en",
    "tiny-q5_1",
    "tiny.en-q5_1",
    "tiny-q8_0",
    "base",
    "base.en",
    "base-q5_1",
    "base.en-q5_1",
    "base-q8_0",
    "small",
    "small.en",
    "small.en-tdrz",
    "small-q5_1",
    "small.en-q5_1",
    "small-q8_0",
    "medium",
    "medium.en",
    "medium-q5_0",
    "medium.en-q5_0",
    "medium-q8_0",
    "large-v1",
    "large-v2",
    "large-v2-q5_0",
    "large-v2-q8_0",
    "large-v3",
    "large-v3-q5_0",
    "large-v3-turbo",
    "large-v3-turbo-q5_0",
    "large-v3-turbo-q8_0",
];

pub fn list_available_models() -> String {
    let mut output = String::new();
    output.push_str("\nAvailable models:");

    let mut current_class = "";
    for model in AVAILABLE_MODELS {
        let model_class = model.split(&['.', '-'][..]).next().unwrap_or("");
        if model_class != current_class {
            output.push_str(&format!("\n {model_class}"));
            current_class = model_class;
        }
        output.push_str(&format!(" {model}"));
    }

    output.push_str("\n\n");
    output.push_str("___________________________________________________________\n");
    output.push_str(".en = english-only  -q5_[01] = quantized  -tdrz = tinydiarize\n");

    output
}

pub fn validate_model(model: &str) -> Result<()> {
    if AVAILABLE_MODELS.contains(&model) {
        Ok(())
    } else {
        Err(anyhow!(
            "Invalid model: {}\n{}",
            model,
            list_available_models()
        ))
    }
}

fn get_download_info(model: &str) -> (String, String) {
    if model.contains("tdrz") {
        (
            "https://huggingface.co/akashmjn/tinydiarize-whisper.cpp".to_string(),
            "resolve/main/ggml".to_string(),
        )
    } else {
        (
            "https://huggingface.co/ggerganov/whisper.cpp".to_string(),
            "resolve/main/ggml".to_string(),
        )
    }
}

fn check_download_tool() -> Result<String> {
    let tools = ["wget2", "wget", "curl"];

    for tool in &tools {
        if Command::new("which")
            .arg(tool)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
        {
            return Ok(tool.to_string());
        }
    }

    Err(anyhow!(
        "Either wget, wget2, or curl is required to download models. Please install one of them."
    ))
}

fn download_with_tool(tool: &str, url: &str, output_path: &str) -> Result<()> {
    let mut cmd = Command::new(tool);

    match tool {
        "wget2" => {
            cmd.args(["--no-config", "--progress", "bar", "-O", output_path, url]);
        }
        "wget" => {
            cmd.args([
                "--no-config",
                "--quiet",
                "--show-progress",
                "-O",
                output_path,
                url,
            ]);
        }
        "curl" => {
            cmd.args(["-L", "--output", output_path, url]);
        }
        _ => return Err(anyhow!("Unsupported download tool: {}", tool)),
    }

    let status = cmd
        .status()
        .map_err(|e| anyhow!("Failed to execute {}: {}", tool, e))?;

    if !status.success() {
        return Err(anyhow!("Download failed with {}", tool));
    }

    Ok(())
}

pub async fn download_model(model: &str, models_path: Option<String>) -> Result<()> {
    // Validate model
    validate_model(model)?;

    // Determine download path
    let download_path = models_path.unwrap_or_else(|| ".".to_string());
    let file_path = Path::new(&download_path).join(format!("ggml-{model}.bin"));

    // Check if model already exists
    if file_path.exists() {
        println!("Model '{model}' already exists. Skipping download.");
        return Ok(());
    }

    // Get download info
    let (src, pfx) = get_download_info(model);
    let url = format!("{src}/{pfx}-{model}.bin");

    println!("Downloading ggml model '{model}' from '{src}'...");

    // Check for download tool
    let tool = check_download_tool()?;

    // Create directory if it doesn't exist
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| anyhow!("Failed to create directory: {}", e))?;
    }

    // Download the model
    download_with_tool(&tool, &url, file_path.to_str().unwrap())?;

    println!("Done! Model '{}' saved in '{}'", model, file_path.display());
    println!("You can now use it like this:");
    println!("  $ open-transcribe file samples/audio.wav");

    Ok(())
}

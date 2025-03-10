use base64;
use dotenv::dotenv;
use reqwest::Client;
use serde_json::json;
use std::env;
use std::fs;
use std::path::Path;

fn get_mime_type(path: &str) -> &'static str {
    let path = Path::new(path);
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenv().ok();

    // Check for command-line argument
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Please provide the path to the image file as a command-line argument.");
        return Ok(());
    }

    let image_path = &args[1];
    let api_key = env::var("GROQ_API_KEY").expect("GROQ_API_KEY environment variable is not set.");

    // Read and encode the image
    let image_bytes = fs::read(image_path)?;
    let base64_image = base64::encode(&image_bytes);

    // Format as data URL with appropriate MIME type
    let mime_type = get_mime_type(image_path);
    let data_url = format!("data:{};base64,{}", mime_type, base64_image);

    let client = Client::new();

    // Vision model request with corrected format
    let message_for_vision = json!({
        "role": "user",
        "content": [
            {"type": "text", "text": "Extract all the text from this image. Only return the exact text you see without any additional explanation, analysis, or context. Do not describe the image or its content, just transcribe any text you can see."},
            {"type": "image_url", "image_url": {"url": data_url}}
        ]
    });

    let request_body_vision = json!({
        "model": "llama-3.2-11b-vision-preview",
        "messages": [message_for_vision],
        "temperature": 0,
        "max_tokens": 1000
    });

    let response_vision = client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body_vision)
        .send()
        .await?;

    if !response_vision.status().is_success() {
        // Save the status before consuming the response
        let status = response_vision.status();
        let error_text = response_vision.text().await?;
        eprintln!(
            "Vision model request failed with status: {}, details: {}",
            status, error_text
        );
        return Ok(());
    }

    let data_vision: serde_json::Value = response_vision.json().await?;
    let extracted_text = data_vision["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("");

    // Text model request
    let message_for_qwen = json!({
        "role": "user",
        "content": format!("Please provide a concise response to this, keeping it short and in the same language as the input: {}", extracted_text)
    });

    let request_body_qwen = json!({
        "model": "qwen-qwq-32b",
        "messages": [message_for_qwen],
        "temperature": 0,
        "max_tokens": 6000
    });

    let response_qwen = client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body_qwen)
        .send()
        .await?;

    if !response_qwen.status().is_success() {
        // Save the status before consuming the response
        let status = response_qwen.status();
        let error_text = response_qwen.text().await?;
        eprintln!(
            "Text model request failed with status: {}, details: {}",
            status, error_text
        );
        return Ok(());
    }

    let data_qwen: serde_json::Value = response_qwen.json().await?;
    let final_answer = data_qwen["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("");

    println!("{}", final_answer);

    Ok(())
}

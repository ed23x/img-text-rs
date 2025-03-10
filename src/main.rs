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
        Some("gif") => "image/gif",
        Some("tif") | Some("tiff") => "image/tiff",
        Some("pdf") => "application/pdf",
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

    // OCR.space API key
    let ocr_api_key = "K85230459188957";

    // Read the image file
    let image_bytes = fs::read(image_path)?;
    let base64_image = base64::encode(&image_bytes);
    let mime_type = get_mime_type(image_path);

    // Format as data URL with appropriate MIME type
    let data_url = format!("data:{};base64,{}", mime_type, base64_image);

    let client = Client::new();

    // Create the OCR.space request with form parameters
    let form_params = [
        ("apikey", ocr_api_key),
        ("base64Image", &data_url),
        ("language", "auto"),
        ("isOverlayRequired", "false"),
        ("OCREngine", "2"), // Using OCR Engine 2 for better accuracy
    ];

    // Send OCR request
    let ocr_response = client
        .post("https://api.ocr.space/parse/image")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&form_params)
        .send()
        .await?;

    if !ocr_response.status().is_success() {
        let status = ocr_response.status();
        let error_text = ocr_response.text().await?;
        eprintln!(
            "OCR request failed with status: {}, details: {}",
            status, error_text
        );
        return Ok(());
    }

    let ocr_data: serde_json::Value = ocr_response.json().await?;

    // Debug output to see the entire OCR response
    println!("OCR Response: {}", serde_json::to_string_pretty(&ocr_data)?);

    // Extract the parsed text from OCR response
    let parsed_text = match &ocr_data["ParsedResults"] {
        serde_json::Value::Array(results) if !results.is_empty() => {
            results[0]["ParsedText"].as_str().unwrap_or("").trim()
        }
        _ => {
            eprintln!("No text found in the OCR response");
            return Ok(());
        }
    };

    if parsed_text.is_empty() {
        println!("No text was extracted from the image.");
        return Ok(());
    }

    println!("\nExtracted text from image: {}\n", parsed_text);

    // Text model request using Groq API
    let groq_api_key =
        env::var("GROQ_API_KEY").expect("GROQ_API_KEY environment variable is not set.");

    let message_for_qwen = json!({
        "role": "user",
        "content": format!("Please provide a concise response to this, keeping it short but show your calculations (in LaTeX) and answer in the same language as the input: {}", parsed_text)
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
        .header("Authorization", format!("Bearer {}", groq_api_key))
        .json(&request_body_qwen)
        .send()
        .await?;

    if !response_qwen.status().is_success() {
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

    println!("Final answer: {}", final_answer);

    Ok(())
}

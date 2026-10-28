fn main() -> Result<(), serde_json::Error> {
    println!(
        "{}",
        serde_json::to_string_pretty(&pipeline_autopsy_api::openapi::document())?
    );
    Ok(())
}

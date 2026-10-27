//! Print the domain schema: cargo run -p pipeline-autopsy-core --example export_schema
fn main() -> Result<(), serde_json::Error> {
    let schema = schemars::schema_for!(pipeline_autopsy_core::Run);
    println!("{}", serde_json::to_string_pretty(&schema)?);
    Ok(())
}

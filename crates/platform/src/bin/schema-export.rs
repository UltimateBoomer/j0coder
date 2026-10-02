fn main() -> anyhow::Result<()> {
    let schema = match std::env::args().nth(1).as_deref() {
        Some("api-components") => j0coder::schema::api_components(),
        Some("authoring") => j0coder::schema::authoring_schema(),
        _ => anyhow::bail!("usage: schema-export <api-components|authoring>"),
    };
    println!("{}", serde_json::to_string_pretty(&schema)?);
    Ok(())
}

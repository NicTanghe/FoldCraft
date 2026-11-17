//! Minimal MCP server for a ClothCraft garment document. CPU only.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    marvelous_cloth::mcp::run_stdio()?;
    Ok(())
}

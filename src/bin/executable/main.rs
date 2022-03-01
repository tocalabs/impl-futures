use execution_engine::start;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    start().await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    locoder::logging();
    locoder::queue::worker(locoder::shutdown::signal()).await
}

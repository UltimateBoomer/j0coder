#[tokio::main]
async fn main() -> anyhow::Result<()> {
    j0coder::logging();
    j0coder::queue::worker(j0coder::shutdown::signal()).await
}

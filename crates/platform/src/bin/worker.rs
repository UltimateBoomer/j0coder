#[tokio::main]
async fn main() -> anyhow::Result<()> {
    practice::logging();
    practice::queue::worker(practice::shutdown::signal()).await
}

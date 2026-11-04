mod log;

use common::Config;
use api as server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    log::init();

    let config = Config::from_env();
    server::init(config).await
}

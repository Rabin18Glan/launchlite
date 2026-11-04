use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn init() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "api=info,launchlite=info".into());

    let formatter = tracing_subscriber::fmt::layer();

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(formatter);

    subscriber.init();
}

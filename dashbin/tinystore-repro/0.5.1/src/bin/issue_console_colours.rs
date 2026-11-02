// one line through tinystore's console logger, to see whether it is coloured in the environment it runs in

use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

fn main() {
    tracing_subscriber::registry().with(tinystore::console::logger("app").layer()).init();
    tracing::warn!(ms = 1200, "slow request");
}

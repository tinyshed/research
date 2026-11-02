// one setting read with config::load, which opens no store, beside a typo's nearest word

#[cfg(feature = "config")]
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Settings {
    port: u16,
}

fn main() {
    #[cfg(feature = "config")]
    {
        let config = tinystore::config::load("app", Settings::default())
            .env(tinystore::config::Env::prefix("APP"))
            .open()
            .unwrap();
        println!("port {}", config.get().port);
    }
    let near = tinystore::typos::Dictionary::new(["serve", "backup"]).nearest("serv", 1);
    println!("nearest {:?}", near.map(|near| near.word));
}

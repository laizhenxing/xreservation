use std::path::Path;

use abi::Config;
use anyhow::Result;

use reservation_service::start_server;

#[tokio::main]
async fn main() -> Result<()> {
    let filename = {
        let p1 = Path::new("./reservation.yml");
        let path = shellexpand::tilde("~/.config/reservation.yml");
        let p2 = Path::new(path.as_ref());
        let p3 = Path::new("/etc/reservation.yml");

        match (p1.exists(), p2.exists(), p3.exists()) {
            (true, _, _) => p1.to_str().unwrap().to_string(),
            (_, true, _) => p2.to_str().unwrap().to_string(),
            (_, _, true) => p3.to_str().unwrap().to_string(),
            _ => panic!("No config file found"),
        }
    };

    let config = Config::load(filename)?;
    start_server(&config).await
}

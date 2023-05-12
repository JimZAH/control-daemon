use serde_derive::{Deserialize, Serialize};
use std::io::Read;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub host: String,
    pub name: String,
    pub topics: Vec<String>,
    pub qos: Vec<i32>,
}

impl Config {
    pub fn load() -> Config {
        /* Unwrapping is fine here because if this fails then we can't run */
        if let Ok(mut f) = std::fs::File::open("config.json") {
            let mut buff = vec![];
            f.read_to_end(&mut buff).unwrap();
            let config = serde_json::from_slice::<Config>(&buff).unwrap();
            return config;
        }
        Self {
            host: "mqtt://10.145.0.4:1883".to_string(),
            name: "GB3VW".to_string(),
            topics: vec![
                "repeater-control".to_string(),
                "repeater-control/status".to_string(),
                "repeater-control/lwt".to_string(),
            ],
            qos: vec![1],
        }
    }
}

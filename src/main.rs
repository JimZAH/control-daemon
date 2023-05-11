use futures::{executor::block_on, stream::StreamExt};
use paho_mqtt as mqtt;
use serde_derive::{Deserialize, Serialize};
use std::{
    env,
    process::{self, Command},
    time::Duration,
};

const TOPICS: &[&str] = &[
    "repeater-control",
    "repeater-control/status",
    "repeater-control/lwt",
];
const QOS: &[i32] = &[1];

/*
 *  Struct which holds command data
 */
#[derive(Serialize, Deserialize)]
struct Cmd {
    command_type: String,
    command: String,
}

struct Config {
    host: String,
    name: String,
}

impl Cmd {
    
    async fn exec(&self, args: Vec<&str>) -> Option<String> {
        if let Ok(c) = Command::new("sh").arg("-c").args(args).output(){
            /* unwrapping should be safe as we should never receive non utf8 from stdout */
            return Some(std::str::from_utf8(&c.stdout).unwrap().to_string())
        }
        None
    } 
    
    async fn process(&self, c: &mqtt::AsyncClient) -> mqtt::Result<()> {
        match self.command_type.chars().next() {
            Some('A') => {
                /*
                 * Testing command
                 */
                if let Some(s) = self.exec(vec!["asterisk", "-x", &self.command]).await{
                    Cmd::response(c, &s).await?
                }
            }
            Some('U') => {
                if let Some(s) = self.exec(vec!["uptime"]).await{
                    Cmd::response(c, &s).await?
                }
            }
            _ => {
                println!("Command not valid");
                Cmd::response(&c, "Command type is not valid").await?
            }
        }
        Ok(())
    }

    async fn response(c: &mqtt::AsyncClient, m: &str) -> mqtt::Result<()> {
        let response = mqtt::message::Message::new(TOPICS[1], m, 1);
        c.publish(response).await
    }
}

fn main() {
    let config = Config {
        host: env::args()
            .nth(1)
            .unwrap_or_else(|| "mqtt://10.145.0.4:1883".to_string()),
        name: "GB3VW".to_string(),
    };

    let create_opts = mqtt::CreateOptionsBuilder::new_v3()
        .server_uri(config.host)
        .client_id(config.name)
        .finalize();

    let mut client = mqtt::AsyncClient::new(create_opts).unwrap_or_else(|e| {
        println!("Error creating the client: {:?}", e);
        process::exit(1);
    });

    if let Err(err) = block_on(async {
        // Get message stream before connecting.
        let mut strm = client.get_stream(25);

        let lwt = mqtt::Message::new(TOPICS[2], "ALIVE", mqtt::QOS_1);

        let conn_opts = mqtt::ConnectOptionsBuilder::new_v3()
            .keep_alive_interval(Duration::from_secs(30))
            .clean_session(false)
            .will_message(lwt)
            .finalize();

        client.connect(conn_opts).await?;

        client.subscribe(TOPICS[0], QOS[0]).await?;

        while let Some(msg_opt) = strm.next().await {
            if let Some(msg) = msg_opt {
                println!("{}", msg);
                if let Ok(cmd) = serde_json::from_str::<Cmd>(&msg.payload_str()) {
                    cmd.process(&client).await?
                }
            } else {
                println!("Lost connection. Attempting reconnect.");
                while let Err(err) = client.reconnect().await {
                    println!("Error reconnecting: {}", err);
                    async_std::task::sleep(Duration::from_millis(1000)).await;
                }
            }
        }

        Ok::<(), mqtt::Error>(())
    }) {
        eprintln!("{}", err);
    }
}

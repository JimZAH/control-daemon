use control::Config;
use futures::{executor::block_on, stream::StreamExt};
use paho_mqtt as mqtt;
use serde_derive::{Deserialize, Serialize};
use std::{
    process::{self, Command},
    time::Duration,
};

const QOS: &[i32] = &[1];

/*
 *  Struct which holds command data
 */
#[derive(Serialize, Deserialize)]
struct Cmd {
    command_type: String,
    command: String,
    response_topic: String,
}

impl Cmd {
    async fn exec(&self, args: Vec<&str>) -> Option<String> {
        if let Ok(c) = Command::new("sh").arg("-c").args(args).output() {
            /* unwrapping should be safe as we should never receive non utf8 from stdout */
            return Some(std::str::from_utf8(&c.stdout).unwrap().to_string());
        }
        None
    }

    async fn process(&self, config: &Config, c: &mqtt::AsyncClient) -> mqtt::Result<()> {
        match self.command_type.chars().next() {
            Some('A') => {
                /*
                 * Testing command
                 */
                if let Some(s) = self.exec(vec!["asterisk", "-x", &self.command]).await {
                    Cmd::response(&config, c, &s).await?
                }
            }
            Some('U') => {
                if let Some(s) = self.exec(vec!["uptime"]).await {
                    Cmd::response(&config, c, &s).await?
                }
            }
            _ => {
                println!("Command not valid");
                Cmd::response(&config, &c, "Command type is not valid").await?
            }
        }
        Ok(())
    }

    async fn response(config: &Config, c: &mqtt::AsyncClient, m: &str) -> mqtt::Result<()> {
        let response = mqtt::message::Message::new(&config.topics[1], m, 1);
        c.publish(response).await
    }
}

fn main() {
    let config = Config::load();

    let create_opts = mqtt::CreateOptionsBuilder::new_v3()
        .server_uri(&config.host)
        .client_id(&config.name)
        .finalize();

    let mut client = mqtt::AsyncClient::new(create_opts).unwrap_or_else(|e| {
        println!("Error creating the client: {:?}", e);
        process::exit(1);
    });

    if let Err(err) = block_on(async {
        // Get message stream before connecting.
        let mut strm = client.get_stream(25);

        let lwt = mqtt::Message::new(&config.topics[2], "ALIVE", mqtt::QOS_1);

        let conn_opts = mqtt::ConnectOptionsBuilder::new_v3()
            .keep_alive_interval(Duration::from_secs(30))
            .clean_session(false)
            .will_message(lwt)
            .finalize();

        client.connect(conn_opts).await?;

        client.subscribe(&config.topics[0], QOS[0]).await?;

        while let Some(msg_opt) = strm.next().await {
            if let Some(msg) = msg_opt {
                println!("{}", msg);
                if let Ok(cmd) = serde_json::from_str::<Cmd>(&msg.payload_str()) {
                    cmd.process(&config, &client).await?
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

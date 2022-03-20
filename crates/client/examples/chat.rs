use bigjson_client::{
    Batch, BigJsonClient, BigJsonClientError, JsonPatch, SubscriptionEvent, SubscriptionStream,
};
use clap::Parser;
use futures_util::StreamExt;
use serde_json::Value;

#[derive(Parser)]
struct Options {
    /// Server url
    #[clap(long = "server", default_value = "http://localhost:3000")]
    server_url: String,
    /// User name
    name: String,
    /// Room name
    room: String,
}

#[tokio::main]
async fn main() {
    let options = Options::parse();
    let client = BigJsonClient::new("http://localhost:3000");
    let room_path = format!("/{}", options.room);

    match client
        .batch(
            Batch::new()
                .test(&room_path, &())
                .add(&room_path, &Vec::<String>::new()),
        )
        .await
    {
        Ok(()) | Err(BigJsonClientError::TestFailed) => {}
        Err(err) => {
            println!("Error: {}", err);
            return;
        }
    }

    match client.subscribe(&room_path).await {
        Ok(stream) => {
            tokio::spawn(receive_messages(stream));
        }
        Err(err) => {
            println!("Error: {}", err);
            return;
        }
    }

    let mut rl = rustyline::Editor::<()>::new();
    let prompt = format!("{}>> ", options.room);

    loop {
        match rl.readline(&prompt) {
            Ok(line) => {
                if let Err(err) = client
                    .add(
                        format!("/{}/-", options.room),
                        &format!("{}: {}", options.name, line),
                    )
                    .await
                {
                    println!("Error: {}", err);
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

async fn receive_messages(mut stream: SubscriptionStream) {
    while let Some(res) = stream.next().await {
        let event = match res {
            Ok(event) => event,
            Err(err) => {
                println!("Error: {}", err);
                return;
            }
        };

        if let SubscriptionEvent::Patch { patch } = event {
            for item in patch {
                match item {
                    JsonPatch::Add {
                        path,
                        value: Value::String(msg),
                    } if path == "/-" => {
                        println!("{}", msg);
                    }
                    _ => {}
                }
            }
        }
    }
}

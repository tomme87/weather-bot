use std::env;
use futures::prelude::*;
use irc::client::prelude::*;
use openweathermap::weather;
use regex::Regex;
use sqlite::Value;

#[tokio::main]
async fn main() -> Result<(), failure::Error> {
    let config = Config::load(format!("{}/config.toml", env::current_dir().unwrap().to_str().unwrap())).unwrap();

    println!("hei");
    let api_key = match config.options.get("openweathermap_api_key") {
        Some(k) => String::from(k),
        None => {
            println!("vaaa");
            panic!("missing api key");
        }
    };

    println!("{}", api_key);

    let connection = sqlite::open(format!("{}/db.sqlite", env::current_dir().unwrap().to_str().unwrap())).unwrap();
    connection.execute("CREATE TABLE IF NOT EXISTS user (name TEXT UNIQUE, last_location TEXT);").unwrap();

    let mut client = Client::from_config(config).await?;
    client.identify()?;

    let mut stream = client.stream()?;

    let re = Regex::new(r"^!v (.+)").unwrap();

    while let Some(message) = stream.next().await.transpose()? {
        print!("{}", message);

        let nick = match message.source_nickname() {
            Some(s) => String::from(s),
            None => String::from("")
        };

        if let Command::PRIVMSG(channel, msg) = message.command {
            if let Some(caps) = re.captures(msg.as_str()) {
                let location = caps.get(1).unwrap().as_str();
                if send_weather(location, &channel, &client, &api_key).await {
                    let mut statement = connection.prepare("INSERT INTO user (name, last_location) VALUES(?,?) ON CONFLICT(name) DO UPDATE SET last_location=?").unwrap();
                    statement.bind(1, nick.as_str()).unwrap();
                    statement.bind(2, location).unwrap();
                    statement.bind(3, location).unwrap();
                    statement.next().unwrap();
                }
            } else if msg.eq("!v") {
                let mut cursor = connection.prepare("SELECT last_location FROM user WHERE name = ?").unwrap().into_cursor();
                cursor.bind(&[Value::String(nick)]).unwrap();
                if let Some(row) = cursor.next().unwrap() {
                    println!("last = {}", row[0].as_string().unwrap());
                    send_weather(row[0].as_string().unwrap(), &channel, &client, &api_key).await;
                }
            }
        }
    }

    Ok(())
}

async fn send_weather(location: &str, channel: &str, client: &Client, api_key: &str) -> bool {
    match &weather(location, "metric", "no", api_key).await {
        Ok(current) => {
            client.send_privmsg(&channel, format!("{} nå: {}°C, {}", current.name.as_str(), current.main.temp, current.weather[0].description.as_str())).unwrap();
            true
        },
        Err(e) => {
            client.send_privmsg(&channel, e).unwrap();
            false
        }
    }
}

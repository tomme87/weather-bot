use std::env;
use futures::prelude::*;
use irc::client::prelude::*;
use regex::{Regex};
use rusqlite::Connection;
use reqwest::Url;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct WeatherData {
    name: String,
    description: String,
    temperature: f64,
}

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

    let connection = Connection::open(format!("{}/db.sqlite", env::current_dir().unwrap().to_str().unwrap())).unwrap();
    connection.execute("CREATE TABLE IF NOT EXISTS user (name TEXT UNIQUE, last_location TEXT)", []).unwrap();

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
                send_weather(location, &channel, &client, &api_key).await.unwrap();
                connection.execute("INSERT INTO user (name, last_location) VALUES(?1,?2) ON CONFLICT(name) DO UPDATE SET last_location=?3", [
                    nick.as_str(),
                    location,
                    location
                ]).unwrap();
            } else if msg.eq("!v") {
                let mut statement = connection.prepare("SELECT last_location FROM user WHERE name = ?").unwrap();
                let mut rows = statement.query([nick.as_str()]).unwrap();
                if let Some(row) = rows.next()? {
                    let location = row.get::<usize, String>(0).unwrap();
                    send_weather(location.as_str(), &channel, &client, &api_key).await.unwrap();
                }
            }
        }
    }

    Ok(())
}

async fn send_weather(location: &str, channel: &str, client: &Client, api_key: &str) -> Result<(), failure::Error> {
    let message = match get_weather(location, api_key).await {
        Ok(weather_data) => format!("{} nå: {}°C, {}", weather_data.name.as_str(), weather_data.temperature, weather_data.description.as_str()),
        Err(err) => err.to_string()
    };

    client.send_privmsg(&channel, message).unwrap();
    Ok(())
}

async fn get_weather(city: &str, api_key: &str) -> Result<WeatherData, Box<dyn std::error::Error>> {
    let url = Url::parse_with_params(
        "http://api.openweathermap.org/data/2.5/weather",
        &[
            ("q", city),
            ("appid", api_key),
            ("units", "metric"),
            ("lang", "no"),
        ],
    )?;

    let response = reqwest::get(url).await?;

    if !response.status().is_success() {
        let response_text = response.text().await?;
        return Err(response_text.into())
    }

    let response_text = response.text().await?;

    let data: serde_json::Value = serde_json::from_str(&response_text)?;

    let name = data["name"].as_str().unwrap().to_string();
    let description = data["weather"][0]["description"].as_str().unwrap().to_string();
    let temperature = data["main"]["temp"].as_f64().unwrap();

    let weather_data = WeatherData {
        name,
        description,
        temperature,
    };

    Ok(weather_data)
}

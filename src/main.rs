use std::{env, fs, process};

use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::{Client, Error};
use serde::Deserialize;

use notify::{notify, NotificationParamsBuilder};

pub mod util;

use util::*;

const REQUEST_URL: &str = "https://api.github.com/notifications";
const ENV_VAR_NAME: &str = "GH_NOTIFIER_TOKEN";

#[derive(Deserialize)]
struct NotificationSubject {
    title: String,
    url: Option<String>,
}

#[derive(Deserialize)]
struct Notification {
    id: String,
    subject: NotificationSubject,
    reason: String,
    updated_at: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // handle command line arguments
    if parse_args() {
        return Ok(());
    }
    // else if no arguments used, proceed with default actions:

    // get token from environment variable
    let token = match env::var(ENV_VAR_NAME) {
        Ok(t) => t,
        Err(err) => {
            let error_text = format!("{} {}", ENV_VAR_NAME, err);
            notify_error(&error_text);
            println!("{}", error_text);
            process::exit(1);
        }
    };

    // get or create local persistence file to save notification ids already shown
    let ids_file_path = get_persistence_file_path();

    // make request to GH notifications API
    let client = Client::new();
    let response = match client
        .get(REQUEST_URL)
        .header(USER_AGENT, "Rust Reqwest")
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            notify_connection_error(&format!("{err}"));
            println!("{}", err);
            process::exit(1);
        }
    };

    // handle unsuccessful responses
    let status = response.status();
    if status != 200 {
        let text = response.text().await?;
        let detail = text.split(' ').collect::<String>();
        notify_connection_error(&format!("{status} {detail}"));
        println!("Error response: {} {}", status, text);
        process::exit(1);
    };

    // read already notified ids from file
    let read_ids_str = match fs::read_to_string(&ids_file_path) {
        Ok(ids) => ids,
        _ => "".to_string(),
    };
    let read_id_strs = read_ids_str.split(",").collect::<Vec<&str>>();

    // handle successful API response
    let response_json: Vec<Notification> = response.json().await?;

    // loop through notifications in response, checking against saved notification ids
    // and display desktop notification if identifier not already saved to file
    let mut new_ids: Vec<String> = Vec::new();
    for notification in &response_json {
        let mut identifier: String = notification.id.to_owned();
        identifier.push_str(&notification.updated_at);
        let check = identifier.clone();
        new_ids.push(identifier);
        if read_id_strs.contains(&check.as_str()) {
            // have already notified about this notification
            continue;
        }

        // build notification parts
        let message = &notification.subject.title;
        let optional_url = notification.subject.url.clone();
        let onclick_url = build_pull_or_issue_url(optional_url);
        let reason_vec = &notification.reason.split("_").collect::<Vec<&str>>();
        let subtitle = reason_vec.join(" ");

        // display notification
        match NotificationParamsBuilder::default()
            .title("New Github Notification")
            .subtitle(subtitle.as_str())
            .message(message.as_str())
            .open(onclick_url.as_str())
            .build()
        {
            Ok(params) => notify(&params),
            Err(err) => {
                dbg!(err);
            }
        }
    }

    // save notified IDs to file system
    let ids_len = new_ids.len();
    if ids_len == 1 {
        match fs::write(&ids_file_path, &new_ids[0]) {
            Ok(_) => (),
            Err(err) => {
                dbg!(err);
            }
        }
    } else if ids_len > 1 {
        let ids_to_write: String = new_ids.join(",");
        match fs::write(&ids_file_path, ids_to_write) {
            Ok(_) => (),
            Err(err) => {
                dbg!(err);
            }
        }
    }
    Ok(())
}

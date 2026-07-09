use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use uuid::Uuid;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let token = env::var("HARBOR_CONTROL_TOKEN").map_err(|_| "HARBOR_CONTROL_TOKEN is required")?;
    let port = env::var("HARBOR_CONTROL_PORT").unwrap_or_else(|_| "19420".into());
    let command = command_from_args(&args)?;
    let mut request = command.as_object().cloned().ok_or("invalid command")?;
    request.insert("id".into(), json!(Uuid::new_v4().to_string()));
    request.insert("token".into(), json!(token));

    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))?;
    serde_json::to_writer(&mut stream, &Value::Object(request))?;
    stream.write_all(b"\n")?;
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response)?;
    let value: Value = serde_json::from_str(&response)?;
    println!("{}", serde_json::to_string_pretty(&value)?);
    if value.get("ok") != Some(&Value::Bool(true)) {
        std::process::exit(1);
    }
    Ok(())
}

fn command_from_args(args: &[String]) -> Result<Value, Box<dyn std::error::Error>> {
    match args.first().map(String::as_str) {
        Some("status") => Ok(json!({ "command": "status" })),
        Some("identity-create") if args.len() >= 3 => Ok(json!({
            "command": "identity_create", "display_name": args[1], "passphrase": args[2],
        })),
        Some("identity-unlock") if args.len() >= 2 => Ok(json!({
            "command": "identity_unlock", "passphrase": args[1],
        })),
        Some("identity-lock") => Ok(json!({ "command": "identity_lock" })),
        Some("network-start") => Ok(json!({ "command": "network_start" })),
        Some("network-stop") => Ok(json!({ "command": "network_stop" })),
        Some("network-peers") => Ok(json!({ "command": "network_peers" })),
        Some("contact-string") => Ok(json!({ "command": "contact_string" })),
        Some("contact-add") if args.len() >= 2 => Ok(json!({
            "command": "contact_add", "contact_string": args[1],
        })),
        Some("permission-grant-all") if args.len() >= 2 => Ok(json!({
            "command": "permission_grant_all", "peer_id": args[1],
        })),
        Some("frontend") if args.len() >= 2 => Ok(json!({
            "command": "frontend", "action": args[1],
            "payload": args.get(2).map(|raw| parse_payload(raw)).transpose()?.unwrap_or(json!({})),
        })),
        Some("shutdown") => Ok(json!({ "command": "shutdown" })),
        _ => Err("usage: harborctl <status|identity-create NAME PASS|identity-unlock PASS|identity-lock|network-start|network-stop|network-peers|contact-string|contact-add STRING|permission-grant-all PEER|frontend ACTION [JSON]|shutdown>".into()),
    }
}

fn parse_payload(raw: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let contents = match raw.strip_prefix('@') {
        Some(path) => fs::read_to_string(path)?,
        None => raw.to_string(),
    };
    Ok(serde_json::from_str(&contents)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_inline_frontend_payload() {
        let value = parse_payload(r#"{"peerIds":["peer-a"]}"#).unwrap();
        assert_eq!(value["peerIds"][0], "peer-a");
    }
}

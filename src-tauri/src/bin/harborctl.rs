use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;
use uuid::Uuid;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let token = read_secret_from_environment("HARBOR_CONTROL_TOKEN")?;
    let port = env::var("HARBOR_CONTROL_PORT").unwrap_or_else(|_| "19420".into());
    let command = command_from_args(&args)?;
    let mut request = command.as_object().cloned().ok_or("invalid command")?;
    request.insert("id".into(), json!(Uuid::new_v4().to_string()));
    request.insert("token".into(), json!(token));

    let address = format!("127.0.0.1:{port}")
        .to_socket_addrs()?
        .next()
        .ok_or("HARBOR_CONTROL_PORT did not resolve to a loopback address")?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(10))?;
    stream.set_read_timeout(Some(Duration::from_secs(35)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
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
        Some("status") if args.len() == 1 => Ok(json!({ "command": "status" })),
        Some("identity-create") if matches!(args.len(), 2 | 3) => Ok(json!({
            "command": "identity_create", "display_name": args[1],
            "passphrase": identity_passphrase(args.get(2))?,
        })),
        Some("identity-unlock") if matches!(args.len(), 1 | 2) => Ok(json!({
            "command": "identity_unlock", "passphrase": identity_passphrase(args.get(1))?,
        })),
        Some("identity-lock") if args.len() == 1 => Ok(json!({ "command": "identity_lock" })),
        Some("network-start") if args.len() == 1 => Ok(json!({ "command": "network_start" })),
        Some("network-stop") if args.len() == 1 => Ok(json!({ "command": "network_stop" })),
        Some("network-peers") if args.len() == 1 => Ok(json!({ "command": "network_peers" })),
        Some("network-addresses") if args.len() == 1 => {
            Ok(json!({ "command": "network_addresses" }))
        }
        Some("network-connect") if args.len() == 2 => Ok(json!({
            "command": "network_connect", "multiaddr": args[1],
        })),
        Some("contact-string") if args.len() == 1 => Ok(json!({ "command": "contact_string" })),
        Some("contact-add") if args.len() == 2 => Ok(json!({
            "command": "contact_add", "contact_string": args[1],
        })),
        Some("contact-request") if args.len() == 2 => Ok(json!({
            "command": "contact_request", "peer_id": args[1],
        })),
        Some("contact-accept") if args.len() == 2 => Ok(json!({
            "command": "contact_accept", "peer_id": args[1],
        })),
        Some("contact-status") if args.len() == 2 => Ok(json!({
            "command": "contact_status", "peer_id": args[1],
        })),
        Some("permission-grant-all") if args.len() == 2 => Ok(json!({
            "command": "permission_grant_all", "peer_id": args[1],
        })),
        Some("frontend") if matches!(args.len(), 2 | 3) => Ok(json!({
            "command": "frontend", "action": args[1],
            "payload": args.get(2).map(|raw| parse_payload(raw)).transpose()?.unwrap_or(json!({})),
        })),
        Some("shutdown") if args.len() == 1 => Ok(json!({ "command": "shutdown" })),
        _ => Err("usage: harborctl <status|identity-create NAME [PASS]|identity-unlock [PASS]|identity-lock|network-start|network-stop|network-peers|network-addresses|network-connect MULTIADDR|contact-string|contact-add STRING|contact-request PEER|contact-accept PEER|contact-status PEER|permission-grant-all PEER|frontend ACTION [JSON]|shutdown>".into()),
    }
}

fn identity_passphrase(argument: Option<&String>) -> Result<String, Box<dyn std::error::Error>> {
    match argument {
        Some(passphrase) => Ok(passphrase.clone()),
        None => read_secret_from_environment("HARBOR_IDENTITY_PASSPHRASE"),
    }
}

fn read_secret_from_environment(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let file_name = format!("{name}_FILE");
    match (env::var_os(name), env::var_os(&file_name)) {
        (Some(_), Some(_)) => Err(format!("set only one of {name} or {file_name}").into()),
        (Some(value), None) => non_empty_secret(value.to_string_lossy().into_owned(), name),
        (None, Some(path)) => read_secret_file(Path::new(&path), &file_name),
        (None, None) => Err(format!("{name} or {file_name} is required").into()),
    }
}

fn read_secret_file(path: &Path, source: &str) -> Result<String, Box<dyn std::error::Error>> {
    let value = fs::read_to_string(path)?;
    non_empty_secret(value.trim_end_matches(['\r', '\n']).to_string(), source)
}

fn non_empty_secret(value: String, source: &str) -> Result<String, Box<dyn std::error::Error>> {
    if value.is_empty() {
        Err(format!("{source} must not be empty").into())
    } else {
        Ok(value)
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

    #[test]
    fn parses_harness_contact_and_network_commands_with_exact_arity() {
        let request = command_from_args(&["contact-request".into(), "peer-a".into()]).unwrap();
        assert_eq!(request["command"], "contact_request");
        assert_eq!(request["peer_id"], "peer-a");

        let status = command_from_args(&["contact-status".into(), "peer-a".into()]).unwrap();
        assert_eq!(status["command"], "contact_status");

        let connect = command_from_args(&[
            "network-connect".into(),
            "/ip4/127.0.0.1/tcp/41000/p2p/peer-a".into(),
        ])
        .unwrap();
        assert_eq!(connect["command"], "network_connect");
        assert!(command_from_args(&["status".into(), "unexpected".into()]).is_err());
    }

    #[test]
    fn secret_files_trim_only_line_endings_and_reject_empty_values() {
        let path = std::env::temp_dir().join(format!("harborctl-secret-{}", Uuid::new_v4()));
        fs::write(&path, "pass phrase with spaces\r\n").unwrap();
        assert_eq!(
            read_secret_file(&path, "test").unwrap(),
            "pass phrase with spaces"
        );
        fs::write(&path, "\n").unwrap();
        assert!(read_secret_file(&path, "test").is_err());
        let _ = fs::remove_file(path);
    }
}

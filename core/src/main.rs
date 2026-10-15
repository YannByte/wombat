use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, Read, Write};
use wombat_core::dto::{OperationError, operation_error};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    op: String,
    args: Value,
}

fn main() {
    wombat_core::configure_allocator();
    let arguments: Vec<_> = std::env::args_os().collect();
    if let Some(position) = arguments.iter().position(|arg| arg == "--daemon-config") {
        let result = (|| -> anyhow::Result<()> {
            let file = arguments.get(position + 1).ok_or_else(|| {
                operation_error("INVALID_ARGUMENT", "Missing service startup configuration")
            })?;
            wombat_core::host_environment::configure(std::path::Path::new(file))?;
            wombat_core::live::serve_resident()
        })();
        if let Err(error) = result {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
        return;
    }
    if std::env::args().any(|arg| arg == "--daemon") {
        if let Err(error) = wombat_core::live::serve_resident() {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
        return;
    }
    if std::env::args().any(|arg| arg == "--serve-usage") {
        if let Err(error) = wombat_core::live::serve() {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
        return;
    }
    if std::env::args().any(|arg| arg == "--version") {
        println!("wombat-core {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let result = (|| -> anyhow::Result<Value> {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        let request: Request = serde_json::from_str(&input)
            .map_err(|e| operation_error("INVALID_ARGUMENT", format!("无效内核请求：{e}")))?;
        if !request.args.is_object() {
            return Err(operation_error("INVALID_ARGUMENT", "内核 args 必须是对象"));
        }
        wombat_core::dispatch(&request.op, &request.args)
    })();
    let response = match result {
        Ok(value) => json!({"ok":true,"value":value}),
        Err(error) => {
            let code = error
                .chain()
                .find_map(|e| e.downcast_ref::<OperationError>().map(|e| e.code))
                .or_else(|| {
                    error
                        .chain()
                        .find_map(|e| e.downcast_ref::<io::Error>())
                        .and_then(|e| match e.kind() {
                            io::ErrorKind::NotFound => Some("ENOENT"),
                            io::ErrorKind::PermissionDenied => Some("EACCES"),
                            _ => None,
                        })
                })
                .unwrap_or("CORE_ERROR");
            let details = error.chain().find_map(|e| {
                e.downcast_ref::<OperationError>()
                    .and_then(|e| e.details.clone())
            });
            json!({"ok":false,"error":format!("{error:#}"),"code":code,"details":details})
        }
    };
    let mut output = io::BufWriter::new(io::stdout().lock());
    if serde_json::to_writer(&mut output, &response).is_err() || output.write_all(b"\n").is_err() {
        std::process::exit(1);
    }
}

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
    if std::env::args().any(|arg| arg == "--version") {
        println!("wombat-core {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let result = (|| -> anyhow::Result<Value> {
        if arguments.len() != 1 {
            return Err(operation_error(
                "INVALID_ARGUMENT",
                "Core accepts --version or one JSON request on stdin",
            ));
        }
        let mut input = String::new();
        io::stdin()
            .take(2 * 1024 * 1024 + 1)
            .read_to_string(&mut input)?;
        if input.len() > 2 * 1024 * 1024 {
            return Err(operation_error(
                "RESOURCE_LIMIT",
                "Core request exceeds 2 MiB",
            ));
        }
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

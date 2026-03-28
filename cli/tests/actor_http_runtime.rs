use assert_cmd::Command;
use std::io::{Read, Write};
use std::net::TcpListener;

#[test]
fn actor_runtime_http_flow_returns_response_body() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let port = listener.local_addr().expect("local addr").port();

    let server = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut req_buf = [0u8; 1024];
            let _ = stream.read(&mut req_buf);
            let body = "hello-from-http";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    let script = format!(
        "let worker = spawn actor {{\n    let req = actor_receive_envelope();\n    let body = http_get_text(req.payload);\n    actor_send(req.sender, body);\n}};\n\nlet client = spawn actor {{\n    actor_send(worker, \"http://127.0.0.1:{}/health\");\n    let response = actor_receive();\n    println(response);\n}};\n\nrun_actors(100);\n",
        port
    );

    let mut tmp = tempfile::NamedTempFile::new().expect("tmp file");
    write!(tmp, "{}", script).expect("write script");
    let path = tmp.path().to_str().expect("utf8 path");

    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("run").arg(path);
    let output = cmd.output().expect("run art script");
    assert!(output.status.success(), "script should run successfully");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(
        stdout.contains("hello-from-http"),
        "expected HTTP body in actor response"
    );

    let _ = server.join();
}

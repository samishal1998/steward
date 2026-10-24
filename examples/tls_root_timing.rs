// Temporary macOS diagnostic: plaintext readiness must not depend on keychain work.
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};
fn main() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming().take(2) {
            let mut stream = stream.unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = [0; 4096];
            let _ = stream.read(&mut bytes);
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        }
    });
    for roots in [true, false] {
        let start = Instant::now();
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .tls_built_in_root_certs(roots)
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        let built = start.elapsed();
        let response = client.get(format!("http://{address}/")).send();
        println!(
            "built_in_roots={roots} build_ms={} total_ms={} HTTP={:?}",
            built.as_millis(),
            start.elapsed().as_millis(),
            response.map(|r| r.status().as_u16())
        );
    }
}

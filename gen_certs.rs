use std::fs;

fn main() {
    // For testing, we'll create minimal self-signed certificate PEM files
    // In production, use: openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes
    
    // This is a placeholder certificate - for real testing, generate actual certs
    let cert_pem = include_str!("../tests/fixtures/test-cert.pem");
    let key_pem = include_str!("../tests/fixtures/test-key.pem");
    
    fs::write("dev-cert.pem", cert_pem)
        .expect("Failed to write certificate");
    fs::write("dev-key.pem", key_pem)
        .expect("Failed to write key");
    
    println!("✓ Test certificates created: dev-cert.pem, dev-key.pem");
}

taskkill /F /IM color_detection_backend.exe 2> $null
cargo clean
cargo run

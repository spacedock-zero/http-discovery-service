# HTTP Discovery Service

A system service that exposes local mDNS services via a JSON API on port `5380`.

## Usage

**Run locally:**

```bash
cargo run -- run
# OR
./http-discovery-service run
```

**Install as Service (Windows/Linux/macOS):**

```bash
# Auto-elevates on Windows if needed
./http-discovery-service install
./http-discovery-service uninstall
```

## API

`GET http://localhost:5380/` returns a JSON list of discovered services.

## Build

```bash
cargo build --release
```

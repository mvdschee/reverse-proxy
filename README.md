# Reverse Proxy

> A small, fast, single-binary reverse proxy with TLS termination and per-host routing. Drop in a TOML file, point DNS at it, done.

<p align="center"><img src="reverse-proxy.jpg" alt="Architecture diagram" /></p>

Built in Rust on top of Cloudflare's [Pingora](https://github.com/cloudflare/pingora), shipped as a hardened container (~few MB, non-root, read-only rootfs). Designed for self-hosting: one process in front of all your services, configured from a single file.

## Why

I normally use `nginxproxy/nginx-proxy` for my self-hosted projects. It works, but every container has to live on the same Docker network with a `VIRTUAL_HOST` label. That couples the proxy to the apps in a way I don't love.

So I wrote my own (by hand, old-school, no AI writing the code) to learn the internals and end up with something that:

- Has **one place** to declare what gets proxied (a TOML file).
- Doesn't care how upstreams are deployed: containers, host processes, anywhere reachable.
- Boots fast, binds two ports, terminates TLS, forwards. That's it.

## Features

| Status | Feature                                                  |
| ------ | -------------------------------------------------------- |
| done   | HTTP & HTTPS listeners (Pingora + BoringSSL)             |
| done   | Per-host routing from a single `config.toml`             |
| done   | Self-signed certificate generation (`rcgen`) on boot     |
| done   | Per-route TLS mode: `self_signed`, `acme`, `none`        |
| done   | HTTP → HTTPS redirect (301) for routes with a cert       |
| done   | Hardened container: non-root, read-only fs, dropped caps |
| done   | Reach apps on the host via `host.docker.internal`        |
| wip    | Let's Encrypt / ACME issuance (`rustls-acme`)            |
| todo   | Structured logs (errors today, access logs next)         |
| todo   | Full test coverage for behavioural guarantees            |
| todo   | Performance benchmarks                                   |

## Quick start

The image is published to GHCR as `ghcr.io/mvdschee/reverse-proxy`. Two things to provide:

1. A `config.toml` describing your routes.
2. A `docker-compose.yml` that mounts it.

**`config.toml`**

```toml
[acme]
email = "you@example.com"

[[routes]]
host      = "app.example.com"
upstream  = "host.docker.internal:3000"
cert_type = "none"

[[routes]]
host      = "api.example.com"
upstream  = "host.docker.internal:8000"
cert_type = "self_signed"
```

**`docker-compose.yml`**

```yaml
services:
   proxy:
      image: ghcr.io/mvdschee/reverse-proxy:latest
      restart: unless-stopped
      ports:
         - "80:8080"
         - "443:8443"
      volumes:
         - ./config.toml:/etc/proxy/config.toml:ro
         - proxy-certs:/var/lib/proxy/certs
      extra_hosts:
         - "host.docker.internal:host-gateway"
      read_only: true
      security_opt:
         - no-new-privileges:true
      cap_drop:
         - ALL
      tmpfs:
         - /tmp

volumes:
   proxy-certs:
```

Then:

```sh
docker compose up -d
```

Point DNS at the host, and traffic on `:80`/`:443` for the configured hosts will be terminated and proxied to the upstreams.

## Configuration

The full schema lives in [`example/example.toml`](example/example.toml). The fields:

| Field                | Required | Description                                                                                           |
| -------------------- | -------- | ----------------------------------------------------------------------------------------------------- |
| `acme.email`         | yes      | Contact email for Let's Encrypt (used once ACME lands; required today even if every route is `none`). |
| `routes[].host`      | yes      | The `Host` header to match (e.g. `app.example.com`).                                                  |
| `routes[].upstream`  | yes      | `host:port` to forward to. Use `host.docker.internal:<port>` to reach the host machine from Docker.   |
| `routes[].cert_type` | no       | `self_signed` (default works on boot), `acme` (WIP), or `none` (HTTP only). Defaults to `acme`.       |

## How it works

```
       ┌──────────────────────────────────────────┐
       │            Pingora process               │
client │  ┌────────────────┐    ┌──────────────┐  │  HTTP
  ───► │  │ TLS termination│ ──►│ Host router  │ ─┼──────►  upstream
       │  └────────────────┘    └──────────────┘  │
       │     (BoringSSL)         (config.toml)    │
       └──────────────────────────────────────────┘
```

On startup the binary:

1. Reads `config.toml` from `CONFIG_PATH`.
2. Ensures `CERT_DIR` exists and is writable.
3. Generates self-signed certs via `rcgen` for any route configured as `self_signed`.
4. Boots Pingora with one HTTPS listener (SNI-routed) and one HTTP listener, both reading the same per-host route table. The HTTP listener 301-redirects to HTTPS for any route that has a cert.

Routing is purely `Host`-based: incoming `Host` header → route entry → forward to upstream over plain HTTP.

## Under the hood

A few decisions worth knowing about if you want to dig in or contribute.

**Pingora + BoringSSL.** The proxy is built on Pingora (the engine behind a chunk of Cloudflare's edge), with the `boringssl` feature instead of the OpenSSL default. That keeps the binary self-contained and avoids dragging system OpenSSL in.

**Why `8080`/`8443` inside the container.** The image runs as user `nonroot` (uid `65532`) with `cap_drop: ALL` and `no-new-privileges`. A process without `CAP_NET_BIND_SERVICE` can't bind to ports below 1024, so the binary listens on `8080`/`8443` and the compose file maps the standard ports onto them. The defaults of the binary itself (`HTTP_PORT`/`HTTPS_PORT` env vars) are `80`/`443`; the Dockerfile overrides them.

**Hardened base image.** Built and runtime images are both [Docker Hardened Images](https://hub.docker.com/hardened-images/catalog) (alpine-base). Runtime image ships only the binary, `musl`, `ca-certs`, and `libgcc_s.so.1` (needed by the dynamically-linked binary's unwinder). No shell, no package manager, no extras.

**Static-ish musl build.** Cross-compiled to `*-unknown-linux-musl`, but with `-crt-static` disabled so that build-script artifacts (notably `bindgen`'s `dlopen` of `libclang`) work. The runtime image ships `musl`, so the binary still runs cleanly.

**No async runtime juggling.** `main()` is synchronous. Pingora owns its own Tokio runtime, so the binary just initializes config, certs, and hands control to Pingora's server loop.

**Per-route TLS modes.** Each route picks its own cert strategy. `none` skips TLS entirely (handy for an internal-only host). `self_signed` writes a fresh cert on every restart. `rcgen`'s defaults give it a ~2000-year validity, so there's no in-process renewal loop; the restart is the rotation. `acme` is wired into the type system but not yet issuing.

## Environment variables

| Var           | Default                   | Purpose                                     |
| ------------- | ------------------------- | ------------------------------------------- |
| `CONFIG_PATH` | _required_                | Path to `config.toml`.                      |
| `CERT_DIR`    | `.certs/`                 | Where generated/issued certs are persisted. |
| `HTTP_PORT`   | `80` (image sets `8080`)  | Port the HTTP listener binds to.            |
| `HTTPS_PORT`  | `443` (image sets `8443`) | Port the HTTPS listener binds to.           |

## Roadmap

In rough priority order:

1. **ACME issuance.** Wire up `rustls-acme` so the `acme` cert_type flips from "wired" to "working".
2. **Structured logging.** Error logs are in place; access logs and a sane structured format come next.
3. **Full test coverage.** Behavioural guarantees (routing, TLS modes, redirect, error paths) backed by tests, not just manual checks.
4. **Performance benchmarks.** Measure baseline throughput/latency vs. nginx, so future changes can be judged against numbers instead of vibes.
5. **Make each module idiomatic.** V1 is correctness-first; polish the internals once the surface is stable.

## AI disclaimer

I'll be upfront on every public project about what was done with AI. For this one, I wanted to write the Rust myself to keep the skills sharp and have 100% understanding of every bit of code. AI was used for:

- Research and tradeoff discussions
- Cleanup of the README and other prose
- Talking through code-level solutions
- Generating the Docker image scaffolding from a spec

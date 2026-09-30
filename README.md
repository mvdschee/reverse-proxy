# reverse-proxy

A single-binary reverse proxy for self-hosting. One TOML file declares your routes; it handles TLS termination and forwarding from there. Built on [Pingora](https://github.com/cloudflare/pingora) + BoringSSL, ships as a hardened container image (non-root, read-only rootfs, no shell).

<p align="center"><img src="reverse-proxy.jpg" alt="A request flowing through the proxy" /></p>

## Why

`nginxproxy/nginx-proxy` was my go-to for a while and it does the job. The thing I never liked: you have to put `VIRTUAL_HOST` labels on every container and keep the Docker network sorted. Always felt a bit wrong, but hey, it was the best we got.

The landscape's changed and there are more options now. What still annoys me: getting a real certificate on an internal network seems like a lot of work. This project automates Let's Encrypt renewal by writing DNS records via the provider's API.

The code is written by hand, no AI. I want to keep my Rust skills in shape and still enjoy writing software.

## Features

- HTTP and HTTPS listeners, both always on
- Routing by `Host` header from a single TOML file
- Self-signed certs generated on boot for `self_signed` routes
- 301 redirect from HTTP → HTTPS for any route that has a cert
- Full ACME / Let's Encrypt: an hourly renewal loop that orders a cert, writes the DNS TXT record via Cloudflare API, and hot-swaps it in without a restart
- Hardened container image: non-root, read-only rootfs, all capabilities dropped, no shell
- Multi-arch images (amd64, arm64) on Docker Hub and GHCR per release

## Quick start

Two things: a `config.toml` and a compose file.

The image is `maxvanderschee/reverse-proxy` on Docker Hub, `ghcr.io/mvdschee/reverse-proxy` on GHCR. Multi-arch per release tag, plus `latest`.

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

# real cert: the renewal loop orders from Let's Encrypt, writes the DNS
# TXT record via Cloudflare, swaps the new cert in. no restart.
[[routes]]
host      = "blog.example.com"
upstream  = "host.docker.internal:2000"
cert_type = "acme"

# DNS challenge. acme routes without a provider are skipped.
# challenge prefix is always _acme-challenge., not configurable.
[routes.dns_provider.cloudflare]
zone_id   = "your-cloudflare-zone-id"
api_token = "your-cloudflare-api-token"
```

**`docker-compose.yml`**

```yaml
services:
   proxy:
      image: maxvanderschee/reverse-proxy:latest
      restart: unless-stopped
      # runs as nonroot (uid 65532), can't bind below 1024, so 8080/8443
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
      healthcheck:
         test: ["CMD", "nc", "-z", "127.0.0.1", "8080"]
         interval: 30s
         timeout: 3s
         retries: 3
         start_period: 5s

volumes:
   proxy-certs:
```

`docker compose up -d`, point DNS at the box, done. Configured hosts get proxied; everything else gets a `421 Misdirected Request`.

## Config

An `[acme]` table and a list of `[[routes]]`. Full annotated schema: [`example/example.toml`](example/example.toml).

| Field                   | Required | What it does                                                                                                                   |
| ----------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `acme.email`            | yes      | Contact email for Let's Encrypt. Parser requires it even if every route is `none`.                                             |
| `routes[].host`         | yes      | The `Host` header to match, e.g. `app.example.com`.                                                                            |
| `routes[].upstream`     | yes      | `host:port` to forward to, over plain HTTP. `host.docker.internal:<port>` reaches apps on the Docker host.                     |
| `routes[].cert_type`    | no       | `none` (HTTP only), `self_signed`, or `acme`. Defaults to `none`.                                                              |
| `routes[].dns_provider` | no       | Only for `acme` routes. Cloudflare only for now (`zone_id` + `api_token`). Routes without one are skipped by the renewal loop. |

A couple of sharp edges:

- `host` is an exact match. A request for `app.example.com:443` will not match a route for `app.example.com`.
- "TLS route" means `cert_type` ≠ `none`. Those get the 301 on the HTTP listener and are served on 443 with whatever's in `CERT_DIR` (`<host>.pem` / `<host>.key`). Self-signed certs appear on boot; ACME certs come from the renewal loop (renews within 30 days of expiry).
- Missing cert files at startup don't stop the proxy. It boots, logs a warning, serves what it can.

## Environment variables

| Variable      | Binary default | In the image             | What it does                                                 |
| ------------- | -------------- | ------------------------ | ------------------------------------------------------------ |
| `CONFIG_PATH` | _none, exits_  | `/etc/proxy/config.toml` | Path to the TOML config. Binary exits without one.           |
| `CERT_DIR`    | `.certs/`      | `/var/lib/proxy/certs`   | Where certs and the ACME account file (`acme_account`) live. |
| `HTTP_PORT`   | `80`           | `8080`                   | Port for the HTTP listener.                                  |
| `HTTPS_PORT`  | `443`          | `8443`                   | Port for the HTTPS listener.                                 |

The image bakes in the right-hand column (Dockerfile sets all four), so the compose file above doesn't repeat them. Listeners always bind `0.0.0.0`.

## Container image

Both stages build on [Docker Hardened Images](https://hub.docker.com/hardened-images/catalog) alpine. The runtime image has the binary, `libgcc_s.so.1`, musl, busybox (for the `nc` healthcheck), and CA certs. No shell, no package manager.

One build quirk: we cross-compile to `*-unknown-linux-musl` with `-crt-static` off. Fully static musl binaries can't `dlopen`, which breaks bindgen's libclang loader at build time. The runtime image ships musl, so the resulting mostly-dynamic binary runs fine.

Since this sits on the public internet:

- **`read_only: true`** — the only writes are certs, and those go to the named volume. A compromised process can't touch its own binary or config.
- **`cap_drop: ALL`** + **`no-new-privileges`** — no capabilities, no setuid escalation. If it gets owned, the blast radius is the process.

## How it works

Implementation notes, for the curious:

- `main()` is synchronous. Pingora owns its own Tokio runtime; the binary loads config, sorts out certs, hands over to the server loop.
- The cert store is a `HashMap<Host, (Cert, Key)>` behind an `ArcSwap`. The renewal loop swaps new certs in atomically — no restart, no dropped connections.
- ACME account credentials persist in `CERT_DIR/acme_account`. A restart reuses the account, doesn't re-register.
- The renewal loop runs in two stages per host: tick _N_ creates the order and writes the DNS-01 TXT record; tick _N_+1 marks the challenge ready, finalizes, swaps the cert in. Dead orders get dropped and retried later.

## Roadmap

Roughly in the order I'm doing them:

1. Access logs and a proper logging story. Errors go to stdout for now.
2. A test suite for the behavioural bits. Routing, redirects, cert handling, error paths.
3. Benchmarks. Baseline numbers against nginx so I can judge changes by measurement instead of vibes.
4. Idiomatic Rust. V1 was correctness-first; the internals get tidied up once the shape is stable.

## A note on AI

I'm upfront about where I use AI. This one, the Rust is hand-written. What AI helped with:

- Researching trade-offs (Pingora vs. the alternatives, BoringSSL vs. rustls, musl vs. glibc)
- Drafting and polishing text.
- The Dockerfile. I have written enough of those. Didn't feel like doing it by hand again.

The code is mine. If you find a bug, that's on me.

## Contributing

- **Open an issue first.** I'd rather talk through what you want and how before reviewing a PR that already fixes a problem.
- **No AI-written code.** The Rust here is hand-written and I'd like it to stay that way. You can use AI to help you understand a change or clean up but I will be a bit sceptical if you have no Rust experience on your profile.

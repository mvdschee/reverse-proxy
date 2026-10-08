# reverse-proxy

A single-binary reverse proxy for self-hosting. One TOML file declares your routes; it handles TLS termination and forwarding from there. Built on [Pingora](https://github.com/cloudflare/pingora) + BoringSSL, ships as a hardened container image (non-root, read-only rootfs, no shell).

<p align="center"><img src="reverse-proxy.jpg" alt="A request flowing through the proxy" /></p>

## Why

`nginxproxy/nginx-proxy` was my go-to for a while, and it does the job. What I never liked is that every container needs a `VIRTUAL_HOST` label and has to share a network with the proxy. That always felt a bit wrong to me.

A lot has changed since then. Building your own proxy is doable now, though it comes with its own challenges, so I built this one on a proven stack. The core idea: keep it simple for self-hosted projects, and still get a legit Let's Encrypt certificate for a box the internet can't reach. Point `vault.example.com` at a LAN IP in your own DNS, and you get proper HTTPS on your home network.

The code is written by hand, no AI. I want to keep my Rust skills sharp, and I still enjoy writing code.

## Features

- HTTP and HTTPS listeners, both always on
- Routing by `Host` header from a single TOML file
- Self-signed certs generated on boot for `self_signed` routes
- 301 redirect from HTTP → HTTPS for any route that has a cert
- Full ACME / Let's Encrypt: a renewal loop (hourly, every 2 minutes while an order is pending) that orders a cert, writes the DNS TXT record via Cloudflare API, and hot-swaps it in without a restart
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

| Field                   | Required | What it does                                                                                                                                                              |
| ----------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `acme.email`            | yes      | Contact email for Let's Encrypt. Parser requires it even if every route is `none`. Use a real one!!!                                                                      |
| `routes[].host`         | yes      | The `Host` header to match, e.g. `app.example.com`.                                                                                                                       |
| `routes[].upstream`     | yes      | `host:port` to forward to, over plain HTTP. `host.docker.internal:<port>` reaches apps on the Docker host.                                                                |
| `routes[].cert_type`    | no       | `none` (HTTP only), `self_signed`, or `acme`. Defaults to `none`.                                                                                                         |
| `routes[].dns_provider` | no       | Only for `acme` routes. Cloudflare only for now (`zone_id` + `api_token`, see [Cloudflare setup](#cloudflare-setup)). Routes without one are skipped by the renewal loop. |

A couple of sharp edges:

- `host` is an exact match. A request for `app.example.com:443` will not match a route for `app.example.com`.
- "TLS route" means `cert_type` ≠ `none`. Those get the 301 on the HTTP listener and are served on 443 with whatever's in `CERT_DIR` (`<host>.<cert_type>.pem` / `<host>.<cert_type>.key`, e.g. `app.example.com.acme.pem`). Each cert type has its own files, so switching a route's `cert_type` never reuses the other type's cert. Self-signed certs are regenerated on every boot; ACME certs come from the renewal loop (renews within 30 days of expiry).
- The proxy boots even when cert files are missing. It logs a warning and serves what it can.
- The cert is picked by SNI. The TLS handshake only succeeds when the client sends SNI and a cert for that host exists.
- The first ACME cert takes a few minutes. Boot tick writes the TXT record, then a tick every 2 minutes finalizes once the record resolves. Until it lands the host is unreachable: HTTP still 301s to HTTPS, and HTTPS has nothing to serve yet.
- It always talks to Let's Encrypt production, no staging, and agrees to their terms for you. Mind the rate limits while testing.
- No `X-Forwarded-For` / `X-Forwarded-Proto` headers are added. The upstream sees the proxy as the client.

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

- `main()` is synchronous. Pingora has its own Tokio runtime; the binary loads config, sorts out certs, hands over to the server loop.
- The cert store is a `HashMap<Host, (Cert, Key)>` behind an `ArcSwap`. The renewal loop swaps new certs in atomically, no restart.
- ACME account credentials persist in `CERT_DIR/acme_account`. A restart reuses the account, doesn't re-register.
- The renewal loop runs in two stages per host: first tick creates the order and writes the DNS-01 TXT record; later ticks (every 2 minutes while pending) wait until the TXT record resolves, then mark the challenge ready, finalize, swap the cert in. Dead orders get dropped and retried later. Pending orders live in memory, so a restart before it finalizes just starts a fresh order.

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

## Cloudflare setup

`acme` routes need two things from Cloudflare: the zone ID and an API token that can edit DNS in that zone.

**Zone ID.** Domains → pick your domain → Overview. It's in the API section towards the bottom. The Account ID sits right next to it and looks the same, don't mix them up.

**API token.**

1. My Profile → API Tokens → Create Token.
2. Use the "Edit zone DNS" template.
3. Permissions: `Zone` → `DNS` → `Edit`. That's the only one needed: the proxy creates and updates records, and it calls `/zones/<zone_id>/dns_records` directly, so it needs `Edit` but not `Zone` → `Read`.
4. Zone Resources: `Include` → `Specific zone` → your domain. Don't hand it all zones.
5. Optional: Client IP Address Filtering to the box's public IP. If you set a TTL, renewals stop when it expires.
6. Create, copy the token (you only see it once), put it in `api_token`.

What the proxy does with it:

- Writes one TXT record, `_acme-challenge.<host>`, TTL auto, not proxied, with a comment so you can spot it.
- Keeps the record around. The next renewal updates it in place. Leave it alone.
- The host has to live in that zone: `blog.example.com` → the `example.com` zone. No wildcards, one cert per host.
- The token is plain text in `config.toml`. Keep that in mind.
- DNS-01 means Let's Encrypt never connects to the box. The A record can point at a LAN IP, that's the whole point.

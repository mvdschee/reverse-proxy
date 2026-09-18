# reverse-proxy

A reverse proxy for self-hosting, in a single binary. You point it at your hosts in a TOML file, it terminates TLS where you ask it to, and forwards to your upstreams. Built on Cloudflare's [Pingora](https://github.com/cloudflare/pingora) (the engine behind a chunk of their edge) with BoringSSL for TLS, and shipped as a hardened container image: non-root, read-only rootfs, no shell.

<p align="center"><img src="reverse-proxy.jpg" alt="A request flowing through the proxy" /></p>

## Why

I run `nginxproxy/nginx-proxy` for my self-hosted projects. It works, but it only talks to apps that live on the same Docker network with `VIRTUAL_HOST` labels, and that coupling between the proxy and my deployment setup never sat right with me.

So I wrote my own (by hand, old-school, no AI writing the code) to learn how a proxy actually works. What came out of it:

- One file declares what gets proxied.
- It doesn't care how your apps are deployed. Container, host process, a machine on the LAN, anything it can reach.
- It boots, binds 80 and 443, terminates TLS, forwards. That's it.

## Features

- HTTP and HTTPS listeners (both always on)
- Routing by `Host` header from a single TOML file
- Self-signed certificates, generated on boot for `self_signed` routes
- A 301 redirect from HTTP to HTTPS for routes that have a cert
- ACME / Let's Encrypt: account creation, an hourly renewal loop, and a Cloudflare DNS provider (end-to-end issuance is still WIP, see the roadmap)
- A hardened container image: non-root, read-only rootfs, all capabilities dropped
- Multi-arch images (amd64, arm64) published to Docker Hub and GHCR per release

## Quick start

You provide two things: a `config.toml` with your hosts, and a compose file.

The image is published as `maxvanderschee/reverse-proxy` on Docker Hub and as `ghcr.io/mvdschee/reverse-proxy` on GHCR. You get multi-arch builds (amd64, arm64) per release tag, plus `latest`.

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

# ACME is still a work in progress (see the roadmap): the account, the
# renewal loop and the DNS provider are in, but an order is never finalized,
# so until that lands this route gets no cert and its HTTPS side won't serve
# TLS. Keep it here as a shape reference, or flip it to self_signed if you
# want this host actually working.
[[routes]]
host      = "blog.example.com"
upstream  = "host.docker.internal:2000"
cert_type = "acme"

# needed for the DNS-01 challenge. the renewal loop skips acme routes that
# don't have a provider. the challenge prefix is always _acme-challenge.,
# it can't be configured.
[routes.dns_provider.cloudflare]
zone_id   = "your-cloudflare-zone-id"
api_token = "your-cloudflare-api-token"
```

**`docker-compose.yml`**

```yaml
services:
   proxy:
      image: maxvanderschee/reverse-proxy:latest # or ghcr.io/mvdschee/reverse-proxy:latest
      restart: unless-stopped
      # the image runs non-root and can't bind 80/443, hence 8080/8443
      # (see "The container image" below)
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

Then `docker compose up -d`, point your DNS at the box, and you're done. Requests for a configured host get proxied, and everything else gets a `421 Misdirected Request`.

## Config

The file is small: an `[acme]` table and a list of `[[routes]]`. The full schema, with comments, lives in [`example/example.toml`](example/example.toml).

| Field                   | Required | What it does                                                                                                                                                                                      |
| ----------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `acme.email`            | yes      | Contact email for the Let's Encrypt account. Required by the parser even if every route is `none`.                                                                                                |
| `routes[].host`         | yes      | The `Host` header to match, e.g. `app.example.com`.                                                                                                                                               |
| `routes[].upstream`     | yes      | The `host:port` to forward to, over plain HTTP. Use `host.docker.internal:<port>` to reach apps running on the Docker host.                                                                       |
| `routes[].cert_type`    | no       | `none` (HTTP only), `self_signed`, or `acme` (WIP, see the roadmap). Defaults to `none`.                                                                                                          |
| `routes[].dns_provider` | no       | Only meaningful for `acme` routes. The DNS provider used for the DNS-01 challenge. Currently Cloudflare only (`zone_id` + `api_token`); `acme` routes without it are skipped by the renewal loop. |

A couple of things worth knowing:

- `host` is matched exactly against the `Host` header. A request for `app.example.com:443` will not match a route for `app.example.com`. That's a deliberate strictness, not a bug.
- A route counts as "TLS" when its `cert_type` is anything other than `none`. TLS routes get the 301 on the HTTP listener, and the HTTPS listener serves them with the cert in `CERT_DIR` (`<host>.pem` / `<host>.key`). `self_signed` routes get one on boot, `acme` routes get one once the issuance flow finishes.
- If a route's cert files are missing at startup, the proxy boots anyway and logs a warning.

## Environment variables

| Variable      | Binary default | In the image             | What it does                                                                     |
| ------------- | -------------- | ------------------------ | -------------------------------------------------------------------------------- |
| `CONFIG_PATH` | _none, exits_  | `/etc/proxy/config.toml` | Path to the TOML config. The binary exits without one.                           |
| `CERT_DIR`    | `.certs/`      | `/var/lib/proxy/certs`   | Where certs are written, and where the ACME account file (`acme_account`) lives. |
| `HTTP_PORT`   | `80`           | `8080`                   | Port for the HTTP listener.                                                      |
| `HTTPS_PORT`  | `443`          | `8443`                   | Port for the HTTPS listener.                                                     |

The image bakes in the right-hand column (the Dockerfile sets all four), which is why the compose file above doesn't set any of them: it just mounts your config at the path the image already expects, and maps `80`/`443` onto `8080`/`8443`. The port remap exists because the container runs as `nonroot` (uid 65532) with all capabilities dropped, and without `CAP_NET_BIND_SERVICE` a process can't bind a port below 1024.

The listeners always bind `0.0.0.0`.

## The container image

Both build and runtime stages use the [Docker Hardened Images](https://hub.docker.com/hardened-images/catalog) alpine base. The runtime image ships the binary, `libgcc_s.so.1` (the binary is dynamically linked and needs the unwinder at runtime), musl, busybox, and CA certificates. There's no shell and no package manager in there.

One build detail that is a little unusual: we cross-compile to `*-unknown-linux-musl` but with `-crt-static` disabled. Fully static musl binaries can't `dlopen`, which breaks bindgen's libclang loader during the build. The runtime image ships musl, so the resulting (mostly dynamic) binary runs fine on it.

The proxy runs on the public internet, so I harden the container:

- `read_only: true` — the proxy only writes its certs, and those go to the named volume. There's no reason the rest of the filesystem should be writable, so if it gets compromised it can't alter its own binary or config.
- `cap_drop: ALL` and `no-new-privileges: true` — run with no capabilities and no setuid escalation. If the proxy is compromised, the damage stays in the process.

## Internals

A few implementation notes if you want to dig in or contribute:

- `main()` is synchronous. Pingora owns its own Tokio runtime, so the binary just loads the config, sorts out the certs, and hands over to the server loop.
- The cert store is a `HashMap` of host to (cert, key) behind an `ArcSwap`. When the renewal loop is finished it can swap in a new cert without a restart. The swap isn't implemented yet; that's the missing piece of the ACME work.
- The ACME account credentials are persisted in `CERT_DIR/acme_account`, so a restart reuses the account instead of registering a new one every time.
- A background task wakes up every hour and drives the renewal loop for any `acme` routes.

## Roadmap

Roughly in the order I'm doing things:

1. **Finish the ACME flow.** Create and verify the DNS challenge records, confirm the record is in place before finalizing, issue a staging cert first, then prod, and swap the renewed cert into the store. Until this is done, `acme` routes won't get real certs.
2. **Access logs and a proper logging story.** Errors go to stdout today.
3. **A test suite for the behavioural bits.** Routing, redirects, cert handling, error paths.
4. **Benchmarks.** Baseline numbers to compare against nginx, so I can judge changes by measurement instead of vibes.
5. **Idiomatic Rust.** V1 was correctness-first. The internals get tidied up once the shape is stable.

## On AI assistance

I'm upfront about how much AI I use on my projects. This one the Rust is written by hand, I wanted to understand every line of it, and this was mostly an excuse to learn how a proxy works. Where I did use AI:

- Researching trade-offs (Pingora vs. the alternatives, BoringSSL vs. Rustls, musl vs. glibc)
- Drafting and polishing this README
- Dockerfile, I have done enough of those. Really did not feel like writing that by hand :)

The code itself is mine, so if you find a bug, that's on me.

## Contributing

- **Open an issue first.** I'd rather talk through what you want to change, and how, in an issue than review a random PR.
- **No AI-written code.** The Rust in this repo is written by hand, and I'd like it to stay that way. It's fine to use AI to help you write a change, but you must understand the code you're changing. This is a Rust project, and your profile should show it.

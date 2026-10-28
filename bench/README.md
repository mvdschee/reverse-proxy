# Benchmarks

Measures what the proxy costs by sending the same load three ways:

| Arm | Path | Shows |
|---|---|---|
| `baseline` | client → nginx `:9000` | the upstream on its own |
| `plain` | client → proxy `:80` → nginx | the proxy's own overhead |
| `tls` | client → proxy `:443` → nginx | proxy plus TLS and h2 |

`plain` minus `baseline` is what proxying costs. `tls` minus `plain` is what TLS
costs.

## Server

You need a Linux box with at least four cores, with the proxy and nginx on the
same machine. Stop other services on it so they don't skew the numbers.

```sh
# once
sudo apt install build-essential cmake clang libclang-dev golang pkg-config nginx linux-perf
sudo systemctl disable --now nginx   # the package starts its own nginx on :80
curl --proto '=https' -sSf https://sh.rustup.rs | sh

# every session
sudo cpupower frequency-set -g performance   # optional, steadier numbers
bench/server.sh
```

`server.sh`:

- builds the proxy with debug symbols
- creates the files nginx serves
- pins nginx to the last two cores and the proxy to the rest, which you can
  override with `NGINX_CPUS` and `PROXY_CPUS`
- writes the proxy's CPU, RSS, FD and thread counts to `results/server.csv`
  every second

Press ctrl-c to stop all of it. The script asks for sudo once, to set
`net.ipv4.ip_unprivileged_port_start=80` so the proxy can bind 80 and 443
without root. That setting lasts until reboot.

## Clients

Use one or more machines on a wired link to the server. Each needs `oha` and
`h2load`. On macOS, run `brew install oha nghttp2`. On Linux, run
`cargo install oha` and `apt install nghttp2-client`. Each client also needs
these hosts entries:

```sh
echo "<server-ip> bench.test bench-plain.test" | sudo tee -a /etc/hosts
```

Check that all three arms answer `ok`:

```sh
curl -s bench.test:9000/tiny; curl -s bench-plain.test/tiny; curl -sk https://bench.test/tiny
```

Then run a scenario. With more than one client, start them at the same moment.

```sh
bench/run.sh tiny
```

| Scenario | What it does |
|---|---|
| `tiny` | concurrency sweep 16 / 64 / 256 / 1024 on a tiny response, all arms |
| `h2` | h2 with few connections × many streams vs many × few |
| `handshake` | a new TLS connection per request |
| `files` | 1, 10 and 100 MB downloads, watch proxy RSS |
| `slow` | 1000 connections to a throttled upstream |
| `upload` | 1 MB POST bodies |
| `errors` | `/error` and `/redirect`, checks for connection leaks |
| `soak` | one hour on `/api`, checks for leaks |

`DURATION` and `WARMUP` set the length in seconds, and default to 30 and 5.

## Results

- `results/<run>.txt` on each client holds the oha or h2load output: RPS,
  latency percentiles and status codes.
- `results/runs.csv` on each client holds `run,start,end` in epoch seconds.
- `results/server.csv` on the server holds one row per second.

To find the server numbers for a run, take its window from `runs.csv` and look
it up in `server.csv`. Both machines need their clocks synced over NTP. With
several clients, add their RPS together, but report p50 and p99 for each
client, because percentiles can't be combined.

## Pitfalls

- On 1 GbE, the 100 MB files saturate the link on every arm, so compare RSS
  there, not throughput.
- If a client's CPU is pegged, the run measured the client and not the proxy.
  Add another client.
- `handshake` runs are short because every request leaves a client port in
  TIME_WAIT.
- For a flamegraph of a run, use DWARF unwinding, because frame pointers give
  broken stacks on Rust release builds:
  `perf record -F 999 --call-graph dwarf -p $(pidof proxy) -- sleep 30`

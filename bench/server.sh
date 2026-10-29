#!/usr/bin/env bash
# starts nginx, the proxy and the sampler on the server, see docs/benchmarks.md
# ctrl-c stops everything
set -euo pipefail
cd "$(dirname "$0")"
ulimit -n "$(ulimit -Hn)"

# nginx gets the last two cores, the proxy the rest
cores=$(nproc)
NGINX_CPUS=${NGINX_CPUS:-$((cores - 2))-$((cores - 1))}
PROXY_CPUS=${PROXY_CPUS:-0-$((cores - 3))}

# what the upstream serves
mkdir -p www/files tmp results
printf '{"items":[%s]}\n' "$(for i in $(seq 1 30); do printf '{"id":%d,"name":"item-%d"},' "$i" "$i"; done | sed 's/,$//')" > www/api.json
{
	echo '<!doctype html><html><head><title>bench</title></head><body>'
	for i in $(seq 1 600); do echo "<p>paragraph $i lorem ipsum dolor sit amet</p>"; done
	echo '</body></html>'
} > www/page.html
for size in 1 10 100; do
	f="www/files/${size}m.bin"
	[ -f "$f" ] || head -c "$((size * 1024 * 1024))" /dev/urandom > "$f"
done

# symbols for perf
(cd .. && CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release)
proxy_bin=../target/release/proxy

# lets the proxy bind 80 and 443 without root, until reboot
# not setcap: file capabilities make the process non-dumpable, which hides /proc/<pid>/fd and blocks perf -p
sudo sysctl -qw net.ipv4.ip_unprivileged_port_start=80
sudo setcap -r "$proxy_bin" 2>/dev/null || true

# debian keeps nginx in /usr/sbin
PATH=$PATH:/usr/sbin

trap 'kill 0' EXIT

taskset -c "$NGINX_CPUS" nginx -p "$PWD" -c "$PWD/bench-upstream.conf" &
CONFIG_PATH=proxy.toml CERT_DIR=.certs ACME_ENV=staging MIMALLOC_ALLOW_THP=0 taskset -c "$PROXY_CPUS" "$proxy_bin" &
pid=$!

# with THP on "always" the kernel grows idle RSS by hundreds of MB
sleep 1
grep -q '^THP_enabled:[[:space:]]*0' "/proc/$pid/status" || {
	echo "THP is still enabled for the proxy, check MIMALLOC_ALLOW_THP" >&2
	exit 1
}
echo "nginx on cpus $NGINX_CPUS, proxy ($pid) on cpus $PROXY_CPUS, sampling to bench/results/server.csv"

# once a second: epoch,cpu_pct,rss_kb,fds,threads
# cpu_pct is per core, so 400 means four cores busy
# fields 14 and 15 of /proc/<pid>/stat are utime and stime in ticks
hz=$(getconf CLK_TCK)
ticks() { awk '{print $14 + $15}' "/proc/$pid/stat"; }
{
	echo "epoch,cpu_pct,rss_kb,fds,threads"
	prev=$(ticks)
	while sleep 1; do
		now=$(ticks) || break
		rss=$(awk '/^VmRSS/ {print $2}' "/proc/$pid/status")
		threads=$(awk '/^Threads/ {print $2}' "/proc/$pid/status")
		fds=$(ls "/proc/$pid/fd" | wc -l)
		echo "$(date +%s),$(((now - prev) * 100 / hz)),$rss,$fds,$threads"
		prev=$now
	done
} > results/server.csv &

wait

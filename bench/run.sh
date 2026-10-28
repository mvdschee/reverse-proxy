#!/usr/bin/env bash
# runs one scenario from the Mac, see docs/benchmarks.md
# needs bench.test and bench-plain.test in /etc/hosts pointing at the server
# usage: bench/run.sh <tiny|h2|handshake|files|slow|upload|errors|soak>
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p results
ulimit -n 65536 2>/dev/null || true
[ -f upload-1m.bin ] || head -c "$((1024 * 1024))" /dev/urandom > upload-1m.bin

BASELINE=http://bench.test:9000
PLAIN=http://bench-plain.test
TLS=https://bench.test
DURATION=${DURATION:-30}
WARMUP=${WARMUP:-5}

# log <name> <start>: appends the run window, to join with server.sh's server.csv
log() {
	echo "$1,$2,$(date +%s)" >> results/runs.csv
}

# load <name> <oha args...>: warm up, then measure for $DURATION seconds
load() {
	local name=$1 start
	shift
	echo "== $name"
	oha --no-tui --insecure --redirect 0 -z "${WARMUP}s" "$@" > /dev/null
	start=$(date +%s)
	oha --no-tui --insecure --redirect 0 -z "${DURATION}s" "$@" | tee "results/$name.txt"
	log "$name" "$start"
}

# h2 <name> <h2load args...>: same as load, for h2 multiplexing
h2() {
	local name=$1 start
	shift
	echo "== $name"
	h2load -D "$WARMUP" "$@" > /dev/null
	start=$(date +%s)
	h2load -D "$DURATION" "$@" | tee "results/$name.txt"
	log "$name" "$start"
}

arms() {
	echo "baseline=$BASELINE plain=$PLAIN tls=$TLS"
}

case ${1:-} in
tiny)
	for c in 16 64 256 1024; do
		for arm in $(arms); do
			load "tiny-${arm%%=*}-c$c" -c "$c" "${arm#*=}/tiny"
		done
	done
	;;
h2)
	h2 "h2-tls-c8-m100" -t 4 -c 8 -m 100 "$TLS/api"
	h2 "h2-tls-c512-m4" -t 4 -c 512 -m 4 "$TLS/api"
	# same total streams over h1 for comparison
	load "h1-tls-c800-api" -c 800 "$TLS/api"
	;;
handshake)
	# short on purpose, every request leaves a client port in TIME_WAIT
	for arm in $(arms); do
		DURATION=10 load "handshake-${arm%%=*}-c64" -c 64 --disable-keepalive "${arm#*=}/tiny"
	done
	;;
files)
	for f in 1m 10m 100m; do
		for c in 16 64; do
			for arm in $(arms); do
				load "files-$f-${arm%%=*}-c$c" -c "$c" "${arm#*=}/files/$f.bin"
			done
		done
	done
	;;
slow)
	for arm in $(arms); do
		DURATION=60 load "slow-${arm%%=*}-c1000" -c 1000 "${arm#*=}/slow/1m.bin"
	done
	;;
upload)
	for arm in $(arms); do
		load "upload-${arm%%=*}-c64" -c 64 -m POST -D upload-1m.bin "${arm#*=}/upload"
	done
	;;
errors)
	for path in error redirect; do
		for arm in $(arms); do
			load "$path-${arm%%=*}-c64" -c 64 "${arm#*=}/$path"
		done
	done
	;;
soak)
	DURATION=${DURATION_SOAK:-3600} load "soak-tls-c256" -c 256 "$TLS/api"
	;;
*)
	echo "usage: $0 <tiny|h2|handshake|files|slow|upload|errors|soak>" >&2
	exit 1
	;;
esac

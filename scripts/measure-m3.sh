#!/bin/sh
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CLI="$ROOT/target/${BIN_DIR:-debug}/local-transcribe"
NODE="$ROOT/target/${BIN_DIR:-debug}/local-transcribe-node"
SERVICE="$ROOT/target/${BIN_DIR:-debug}/voice-to-text-service"
APP_DATA="$HOME/Library/Application Support/com.timmy.voicetotextapp"
ZIP="$APP_DATA/models/sherpa-onnx-x-asr-zipformer-transducer-zh-en-int8-2026-06-03"
FIRE="$APP_DATA/models/sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26"
WORK="/tmp/ltm3-$$"
mkdir -p "$WORK/a" "$WORK/b" "$WORK/node"
cp "$APP_DATA/selected-model.json" "$WORK/b/selected-model.json"
printf '%s' '{"mode":"local_only","nodes":[{"id":"loop","name":"loop","host":"127.0.0.1","port":7899,"model_id":null,"enabled":true}]}' > "$WORK/a/lan-settings.json"
ffmpeg -loglevel error -f lavfi -i anullsrc=r=16000:cl=mono -t 10 -c:a pcm_s16le "$WORK/audio.wav"
nohup "$NODE" --name loop --data-dir "$WORK/node" --listen 127.0.0.1:7899 --compute on --model-dir "$FIRE" --idle-unload 120 > "$WORK/node.log" 2>&1 &
NODE_PID=$!
nohup "$SERVICE" serve --data-dir "$WORK/a" --allow-root "$WORK" --allow-root "/private$WORK" > "$WORK/a.log" 2>&1 &
A_PID=$!
nohup "$SERVICE" serve --data-dir "$WORK/b" --allow-root "$WORK" --allow-root "/private$WORK" > "$WORK/b.log" 2>&1 &
B_PID=$!
trap 'kill "$NODE_PID" "$A_PID" "$B_PID" 2>/dev/null || true' EXIT
for i in $(seq 1 40); do [ -S "$WORK/a/service.sock" ] && [ -S "$WORK/b/service.sock" ] && break; sleep 0.25; done
sleep 2
R=$("$CLI" jobs add "$WORK/audio.wav" --root "$WORK" --data-dir "$WORK/a" --node loop | python3 -c 'import json,sys;print(json.load(sys.stdin)["job"]["jobId"])')
L=$("$CLI" jobs add "$WORK/audio.wav" --root "$WORK" --data-dir "$WORK/b" | python3 -c 'import json,sys;print(json.load(sys.stdin)["job"]["jobId"])')
for i in $(seq 1 120); do
  RS=$("$CLI" jobs show "$R" --data-dir "$WORK/a" | python3 -c 'import json,sys;print(json.load(sys.stdin)["job"]["state"])')
  LS=$("$CLI" jobs show "$L" --data-dir "$WORK/b" | python3 -c 'import json,sys;print(json.load(sys.stdin)["job"]["state"])')
  case "$RS" in completed|failed|cancelled) case "$LS" in completed|failed|cancelled) break;; esac;; esac
  sleep 0.5
done
echo "node_pid=$NODE_PID local_pid=$B_PID states=$RS/$LS"
elapsed=0
ps -o pid=,rss= -p "$NODE_PID" -p "$B_PID" | awk -v e=0 '{print "t="e"s pid="$1" rss_kb="$2}'
for i in $(seq 1 26); do
  sleep 10
  elapsed=$((elapsed + 10))
  ps -o pid=,rss= -p "$NODE_PID" -p "$B_PID" | awk -v e="$elapsed" '{print "t="e"s pid="$1" rss_kb="$2}'
done

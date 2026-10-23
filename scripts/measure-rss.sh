#!/bin/sh
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CLI="$ROOT/target/debug/local-transcribe"
NODE="$ROOT/target/debug/local-transcribe-node"
SERVICE="$ROOT/target/debug/voice-to-text-service"
APP_DATA="$HOME/Library/Application Support/com.timmy.voicetotextapp"
ZIP="$APP_DATA/models/sherpa-onnx-x-asr-zipformer-transducer-zh-en-int8-2026-06-03"
FIRE="$APP_DATA/models/sherpa-onnx-fire-red-asr2-zh_en-int8-2026-02-26"
WORK="/tmp/ltrss-$$"
mkdir -p "$WORK/a" "$WORK/b" "$WORK/node"
cp "$APP_DATA/selected-model.json" "$WORK/b/selected-model.json"
printf '%s' '{"mode":"local_only","nodes":[{"id":"loop","name":"loop","host":"127.0.0.1","port":7899,"model_id":null,"enabled":true}]}' > "$WORK/a/lan-settings.json"
ffmpeg -loglevel error -f lavfi -i anullsrc=r=16000:cl=mono -t 600 -c:a pcm_s16le "$WORK/audio.wav"
nohup "$NODE" --name loop --data-dir "$WORK/node" --listen 127.0.0.1:7899 --compute on --model-dir "$FIRE" --idle-unload 120 > "$WORK/node.log" 2>&1 &
NODE_PID=$!
nohup "$SERVICE" serve --data-dir "$WORK/a" --allow-root "$WORK" --allow-root "/private$WORK" > "$WORK/a.log" 2>&1 &
A_PID=$!
nohup "$SERVICE" serve --data-dir "$WORK/b" --allow-root "$WORK" --allow-root "/private$WORK" > "$WORK/b.log" 2>&1 &
B_PID=$!
trap 'kill "$NODE_PID" "$A_PID" "$B_PID" 2>/dev/null || true' EXIT
for i in $(seq 1 40); do [ -S "$WORK/a/service.sock" ] && [ -S "$WORK/b/service.sock" ] && break; sleep 0.25; done
sleep 2
curl -sf -m 3 http://127.0.0.1:7899/api/v1/health >/dev/null
REMOTE=$("$CLI" jobs add "$WORK/audio.wav" --root "$WORK" --data-dir "$WORK/a" --node loop | python3 -c 'import json,sys;print(json.load(sys.stdin)["job"]["jobId"])')
for i in $(seq 1 120); do
  N=$(curl -sf -m 2 http://127.0.0.1:7899/api/v1/tasks | python3 -c 'import json,sys;t=json.load(sys.stdin);print(sum(1 for x in t if x["state"] in ("running","uploading","queued")))' 2>/dev/null || echo 0)
  [ "$N" != "0" ] && break
  sleep 0.5
done
LOCAL=$("$CLI" jobs add "$WORK/audio.wav" --root "$WORK" --data-dir "$WORK/b" | python3 -c 'import json,sys;print(json.load(sys.stdin)["job"]["jobId"])')
echo "remote=$REMOTE local=$LOCAL"
MAX_NODE=0; MAX_LOCAL=0; MAX_SUM=0; SAMPLES=0
i=0
while [ $i -lt 1200 ]; do
  set -- $(ps -o rss= -p "$NODE_PID" -p "$B_PID" 2>/dev/null)
  if [ $# -ge 2 ]; then
    NODE_RSS=$1; LOCAL_RSS=$2; SUM=$((NODE_RSS + LOCAL_RSS)); SAMPLES=$((SAMPLES+1))
    [ "$NODE_RSS" -gt "$MAX_NODE" ] && MAX_NODE=$NODE_RSS
    [ "$LOCAL_RSS" -gt "$MAX_LOCAL" ] && MAX_LOCAL=$LOCAL_RSS
    [ "$SUM" -gt "$MAX_SUM" ] && MAX_SUM=$SUM
  fi
  RS=$("$CLI" jobs show "$REMOTE" --data-dir "$WORK/a" | python3 -c 'import json,sys;print(json.load(sys.stdin)["job"]["state"])')
  LS=$("$CLI" jobs show "$LOCAL" --data-dir "$WORK/b" | python3 -c 'import json,sys;print(json.load(sys.stdin)["job"]["state"])')
  case "$RS:$LS" in completed:*|failed:*|cancelled:*) case "$LS" in completed|failed|cancelled) break;; esac;; esac
  sleep 1
  i=$((i+1))
done
echo "M2 samples=$SAMPLES node_peak_kb=$MAX_NODE local_peak_kb=$MAX_LOCAL simultaneous_peak_sum_kb=$MAX_SUM remote=$RS local=$LS"
GUI_PID=$(pgrep -f "Local Transcribe.app/Contents/MacOS/voice-to-text-desktop" | head -1)
echo "idle wait 130s..."
sleep 130
ps -o pid=,rss= -p "$NODE_PID" -p "$B_PID" -p "${GUI_PID:-0}" 2>/dev/null | awk '{print "M3 pid="$1" rss_kb="$2}'
echo "work=$WORK"

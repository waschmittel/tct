#!/bin/sh
# Extract ttyd's built-in index.html and inject docker/pwa/head.html before </head>.
set -eu

HEAD_FILE="${1:-/usr/share/tct-pwa/head.html}"
OUT_FILE="${2:-/usr/share/tct-pwa/index.html}"

ttyd --interface 127.0.0.1 --port 17681 true &
TTYD_PID=$!

for _ in $(seq 1 30); do
    if curl --compressed -fsSL http://127.0.0.1:17681/ -o /tmp/ttyd.html 2>/dev/null; then
        break
    fi
    sleep 0.1
done

kill "$TTYD_PID" 2>/dev/null || true
test -s /tmp/ttyd.html

awk -v head_file="$HEAD_FILE" '
{
    idx = index($0, "</head>")
    if (idx > 0) {
        printf "%s", substr($0, 1, idx - 1)
        while ((getline line < head_file) > 0) {
            print line
        }
        close(head_file)
        print substr($0, idx)
    } else {
        print $0
    }
}
' /tmp/ttyd.html > "$OUT_FILE"

rm -f /tmp/ttyd.html "$HEAD_FILE"
